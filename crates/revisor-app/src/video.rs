//! Inline video player. GitHub embeds a video as a bare attachment URL on its own line;
//! [`Videos`] turns such a paragraph into a [`VideoPlayer`] in any markdown `TextView`.
//! macOS only: AVPlayer decodes and plays audio, GPUI's `surface` draws the frames.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    slider::{Slider, SliderEvent, SliderState},
    text::{markdown_ast as mdast, MarkdownNode, MarkdownParseContext, MarkdownPlugin},
    *,
};
use crate::backend;
use gpui_kit::{black, prelude::*, white, *};
use std::{collections::HashMap, path::PathBuf};

/// GitHub attachment URL. These load only with the user's token (private repos 404 without it).
fn is_github_attachment(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    let Some(rest) = path.strip_prefix("https://github.com/") else { return false };
    rest.starts_with("user-attachments/assets/")
        // Older uploads: github.com/<owner>/<repo>/assets/<user id>/<uuid>
        || rest.split('/').nth(2) == Some("assets")
}

/// Whether `url` alone on a line is a video: GitHub attachment URLs (images use `<img>`) or a video file.
pub fn is_video_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    is_github_attachment(url)
        || (path.starts_with("https://") && [".mp4", ".mov", ".m4v", ".webm"].iter().any(|ext| path.ends_with(ext)))
}

/// Text that puts `url` in a paragraph of its own after `before`, so it renders as a player.
pub fn video_block(before: &str, url: &str) -> String {
    let lead = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{lead}{url}\n")
}

/// `m:ss` for a position in seconds.
fn clock(secs: f64) -> String {
    let secs = secs.max(0.) as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Local copies of videos uploaded in this session, by URL. Private-repo URLs only load in a
/// github.com browser session, so the player uses these instead.
#[derive(Default)]
struct LocalVideos(HashMap<String, PathBuf>);

impl Global for LocalVideos {}

pub fn remember_local(url: String, path: PathBuf, cx: &mut App) {
    cx.default_global::<LocalVideos>().0.insert(url, path);
}

/// Markdown plugin: a paragraph that is only a bare video URL becomes a [`VideoPlayer`].
pub struct Videos;

impl MarkdownPlugin for Videos {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "video"
    }

    fn parse(&self, node: &mdast::Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::Paragraph(p) = node else { return None };
        let [child] = p.children.as_slice() else { return None };
        let url = match child {
            // Only autolinks: `[file.pdf](url)` is a file link, not a video.
            mdast::Node::Link(link) => match link.children.as_slice() {
                [mdast::Node::Text(t)] if t.value == link.url => link.url.as_str(),
                _ => return None,
            },
            mdast::Node::Text(t) => t.value.trim(),
            _ => return None,
        };
        is_video_url(url).then(|| MarkdownNode::new("video", SharedString::from(url.to_string())).text(url.to_string()))
    }

    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let url = node.data::<SharedString>().cloned().unwrap_or_default();
        window.use_keyed_state(ElementId::Name(url.clone()), cx, |window, cx| VideoPlayer::new(url, window, cx))
    }
}

pub struct VideoPlayer {
    url: SharedString,
    /// Playback position, 0..1.
    seek: Entity<SliderState>,
    #[cfg(target_os = "macos")]
    playback: Option<mac::Playback>,
    error: Option<String>,
    /// Asking GitHub for a signed link to the video.
    resolving: bool,
    /// The player covers the window, which is made full screen.
    fullscreen: bool,
    /// The window was full screen before, so leaving the player must not change it.
    window_was_fullscreen: bool,
    focus: FocusHandle,
    /// Frames left to pull while paused, so a seek shows its frame.
    refresh: u8,
    _subscription: Subscription,
}

impl VideoPlayer {
    fn new(url: SharedString, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let seek = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.001));
        let subscription = cx.subscribe(&seek, |this: &mut Self, _, event: &SliderEvent, cx| match event {
            SliderEvent::Change(v) | SliderEvent::Release(v) => this.seek_to(v.start() as f64, cx),
        });
        Self {
            url,
            seek,
            #[cfg(target_os = "macos")]
            playback: None,
            error: None,
            resolving: false,
            fullscreen: false,
            window_was_fullscreen: false,
            focus: cx.focus_handle(),
            refresh: 0,
            _subscription: subscription,
        }
    }

    #[cfg(target_os = "macos")]
    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.resolving {
            return;
        }
        if self.playback.is_none() {
            self.error = None;
            let local = cx.try_global::<LocalVideos>().and_then(|g| g.0.get(self.url.as_ref())).cloned();
            if local.is_none() && is_github_attachment(&self.url) {
                // AVPlayer cannot send the token, so get a signed link first.
                // ponytail: the link expires after 5 min; seeking in a longer video after that fails.
                self.resolving = true;
                let url = self.url.to_string();
                backend::spawn(
                    window,
                    cx,
                    move |s| async move { s.provider().await?.resolve_attachment(&url).await.map_err(backend::err) },
                    |this: &mut Self, result, _, _| {
                        this.resolving = false;
                        match result.and_then(|signed| mac::Playback::open(&signed, None)) {
                            Ok(playback) => {
                                playback.toggle();
                                this.playback = Some(playback);
                            }
                            Err(e) => this.error = Some(e),
                        }
                    },
                )
                .detach();
                cx.notify();
                return;
            }
            match mac::Playback::open(&self.url, local) {
                Ok(playback) => self.playback = Some(playback),
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(playback) = &self.playback {
            playback.toggle();
        }
        cx.notify();
    }

    #[cfg(not(target_os = "macos"))]
    fn toggle(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        cx.open_url(&self.url);
    }

    fn set_fullscreen(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if on == self.fullscreen {
            return;
        }
        if on {
            self.window_was_fullscreen = window.is_fullscreen();
            self.focus.focus(window, cx);
        }
        if !self.window_was_fullscreen && window.is_fullscreen() != on {
            window.toggle_fullscreen();
        }
        self.fullscreen = on;
        cx.notify();
    }

    fn seek_to(&mut self, fraction: f64, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        if let Some(playback) = &self.playback {
            playback.seek(fraction);
            self.refresh = 30;
            cx.notify();
        }
        let _ = (fraction, cx);
    }
}

impl Render for VideoPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(target_os = "macos")]
        let (frame, playing, position, duration, ratio) = match &mut self.playback {
            Some(playback) => match playback.poll() {
                Ok(state) => {
                    if state.playing || state.loading || self.refresh > 0 {
                        self.refresh = self.refresh.saturating_sub(1);
                        window.request_animation_frame();
                    }
                    if state.playing && state.duration > 0. {
                        let fraction = (state.position / state.duration) as f32;
                        self.seek.update(cx, |seek, cx| seek.set_value(fraction, window, cx));
                    }
                    (playback.frame.clone(), state.playing, state.position, state.duration, state.ratio)
                }
                Err(e) => {
                    self.error = Some(e);
                    self.playback = None;
                    (None, false, 0., 0., 16. / 9.)
                }
            },
            None => (None, false, 0., 0., 16. / 9.),
        };
        #[cfg(not(target_os = "macos"))]
        let (playing, position, duration, ratio) = (false, 0., 0., 16. / 9.);

        let theme = cx.theme();
        let fullscreen = self.fullscreen;
        let video = div()
            .id("video-frame")
            .relative()
            .bg(black())
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
            .map(|el| {
                #[cfg(target_os = "macos")]
                let el = el.children(frame.map(|frame| surface(frame).size_full()));
                el
            })
            .when(!playing, |el| {
                el.child(
                    div().absolute().inset_0().flex().items_center().justify_center().child(
                        div()
                            .size_12()
                            .rounded_full()
                            .bg(black().opacity(0.6))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::Play).size_6().text_color(white())),
                    ),
                )
            });
        let controls = h_flex()
            .gap_2()
            .px_2()
            .py_1()
            .bg(theme.secondary)
            .child(
                Button::new("video-toggle")
                    .ghost()
                    .xsmall()
                    .icon(if playing { IconName::Pause } else { IconName::Play })
                    .loading(self.resolving)
                    .tooltip(if playing { "Pause" } else { "Play" })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx))),
            )
            .child(div().flex_1().child(Slider::new(&self.seek)))
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} / {}", clock(position), clock(duration))))
            .child(
                Button::new("video-fullscreen")
                    .ghost()
                    .xsmall()
                    .icon(if fullscreen { IconName::Minimize } else { IconName::Maximize })
                    .tooltip(if fullscreen { "Exit full screen (Esc)" } else { "Full screen" })
                    .on_click(cx.listener(|this, _, window, cx| this.set_fullscreen(!this.fullscreen, window, cx))),
            );
        let error = self.error.clone().map(|e| {
            div().px_2().pb_1().text_xs().text_color(theme.danger).child(format!("Cannot play video: {e}"))
        });

        let card = v_flex().my_2().w_full().max_w(px(640.)).rounded_md().overflow_hidden().border_1().border_color(theme.border);
        if !fullscreen {
            return card.child(video.w_full().aspect_ratio(ratio)).child(controls).children(error).into_any_element();
        }
        // Keep the inline slot so the page does not jump; the player covers the whole window above everything.
        let viewport = window.viewport_size();
        card.child(div().w_full().aspect_ratio(ratio).bg(black()))
            .child(deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    v_flex()
                        .id("video-fullscreen-overlay")
                        .track_focus(&self.focus)
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "escape" {
                                this.set_fullscreen(false, window, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .occlude()
                        .w(viewport.width)
                        .h(viewport.height)
                        .bg(black())
                        .child(video.flex_1().w_full())
                        .child(controls)
                        .children(error),
                ),
            )
            .with_priority(100))
            .into_any_element()
    }
}

#[cfg(target_os = "macos")]
impl Drop for VideoPlayer {
    fn drop(&mut self) {
        if let Some(playback) = &self.playback {
            playback.pause();
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use core_foundation::base::TCFType as _;
    use core_video::pixel_buffer::CVPixelBuffer;
    use objc2::{rc::Retained, runtime::AnyObject, AnyThread as _, MainThreadMarker};
    use objc2_av_foundation::{AVPlayer, AVPlayerItem, AVPlayerItemStatus, AVPlayerItemVideoOutput};
    use objc2_core_media::{CMTime, CMTimeFlags};
    use objc2_foundation::{ns_string, NSDictionary, NSNumber, NSString, NSURL};
    use std::path::PathBuf;

    /// GPUI's Metal renderer only draws this format (`'420f'`); anything else panics.
    const PIXEL_FORMAT_420F: u32 = 0x3432_3066;

    pub struct Playback {
        player: Retained<AVPlayer>,
        item: Retained<AVPlayerItem>,
        output: Retained<AVPlayerItemVideoOutput>,
        /// Last decoded frame; kept while paused.
        pub frame: Option<CVPixelBuffer>,
    }

    pub struct State {
        pub playing: bool,
        pub loading: bool,
        pub position: f64,
        pub duration: f64,
        /// Width / height.
        pub ratio: f32,
    }

    fn seconds(t: CMTime) -> f64 {
        if t.flags.contains(CMTimeFlags::Valid) && t.timescale > 0 {
            t.value as f64 / t.timescale as f64
        } else {
            0.
        }
    }

    impl Playback {
        pub fn open(url: &str, local: Option<PathBuf>) -> Result<Self, String> {
            let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
            let url = match local.as_deref().and_then(|p| p.to_str()) {
                Some(path) => NSURL::fileURLWithPath(&NSString::from_str(path)),
                None => NSURL::URLWithString(&NSString::from_str(url)).ok_or("invalid URL")?,
            };
            // kCVPixelBuffer{PixelFormatType,MetalCompatibility,IOSurfaceProperties}Key: the
            // Metal texture cache GPUI uses needs IOSurface-backed, Metal-compatible buffers.
            let format = NSNumber::new_u32(PIXEL_FORMAT_420F);
            let metal = NSNumber::new_bool(true);
            let io_surface = NSDictionary::<NSString, AnyObject>::new();
            let values: [&AnyObject; 3] = [format.as_ref(), metal.as_ref(), io_surface.as_ref()];
            let attributes = NSDictionary::from_slices(
                &[ns_string!("PixelFormatType"), ns_string!("MetalCompatibility"), ns_string!("IOSurfaceProperties")],
                &values,
            );
            unsafe {
                let item = AVPlayerItem::playerItemWithURL(&url, mtm);
                let output = AVPlayerItemVideoOutput::initWithPixelBufferAttributes(
                    AVPlayerItemVideoOutput::alloc(),
                    Some(&attributes),
                );
                item.addOutput(&output);
                let player = AVPlayer::playerWithPlayerItem(Some(&item), mtm);
                Ok(Self { player, item, output, frame: None })
            }
        }

        /// Pull the current frame and report the state; `Err` once the item failed to load.
        pub fn poll(&mut self) -> Result<State, String> {
            unsafe {
                let status = self.item.status();
                if status == AVPlayerItemStatus::Failed {
                    return Err(self.item.error().map_or("unknown error".into(), |e| e.localizedDescription().to_string()));
                }
                let now = self.item.currentTime();
                if self.output.hasNewPixelBufferForItemTime(now) {
                    if let Some(buffer) = self.output.copyPixelBufferForItemTime_itemTimeForDisplay(now, std::ptr::null_mut()) {
                        // `copy…` returns +1; the GPUI-side wrapper takes over that reference.
                        self.frame = Some(CVPixelBuffer::wrap_under_create_rule(Retained::into_raw(buffer).cast()));
                    }
                }
                let size = self.item.presentationSize();
                let ratio = if size.width > 0. && size.height > 0. { (size.width / size.height) as f32 } else { 16. / 9. };
                Ok(State {
                    playing: self.player.rate() != 0.,
                    loading: status == AVPlayerItemStatus::Unknown,
                    position: seconds(now),
                    duration: seconds(self.item.duration()),
                    ratio,
                })
            }
        }

        pub fn toggle(&self) {
            unsafe {
                if self.player.rate() != 0. {
                    self.player.pause();
                    return;
                }
                let (now, end) = (seconds(self.item.currentTime()), seconds(self.item.duration()));
                if end > 0. && now >= end - 0.05 {
                    self.seek(0.);
                }
                self.player.play();
            }
        }

        pub fn pause(&self) {
            unsafe { self.player.pause() }
        }

        pub fn seek(&self, fraction: f64) {
            unsafe {
                let at = fraction.clamp(0., 1.) * seconds(self.item.duration());
                self.player.seekToTime(CMTime {
                    value: (at * 600.) as i64,
                    timescale: 600,
                    flags: CMTimeFlags::Valid,
                    epoch: 0,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{clock, is_github_attachment, is_video_url, video_block};

    #[test]
    fn detects_video_urls() {
        assert!(is_video_url("https://github.com/user-attachments/assets/1b2c-3d"));
        assert!(is_video_url("https://github.com/acme/app/assets/123/1b2c-3d"));
        assert!(is_video_url("https://user-images.githubusercontent.com/1/demo.MP4?x=1"));
        assert!(!is_video_url("https://github.com/acme/app/pull/1"));
        assert!(!is_video_url("https://example.com/shot.png"));
        assert!(!is_video_url("http://example.com/demo.mp4"));
    }

    #[test]
    fn only_github_attachments_get_the_token() {
        assert!(is_github_attachment("https://github.com/user-attachments/assets/f2f3-e0b3"));
        assert!(is_github_attachment("https://github.com/acme/app/assets/123/1b2c"));
        assert!(!is_github_attachment("https://github.com.evil.io/user-attachments/assets/x"));
        assert!(!is_github_attachment("https://example.com/user-attachments/assets/x.mp4"));
        assert!(!is_github_attachment("https://github.com/acme/app/pull/1"));
    }

    #[test]
    fn video_gets_own_paragraph() {
        assert_eq!(video_block("", "u"), "u\n");
        assert_eq!(video_block("hi\n\n", "u"), "u\n");
        assert_eq!(video_block("hi\n", "u"), "\nu\n");
        assert_eq!(video_block("hi", "u"), "\n\nu\n");
    }

    #[test]
    fn clock_format() {
        assert_eq!(clock(0.), "0:00");
        assert_eq!(clock(75.9), "1:15");
        assert_eq!(clock(-1.), "0:00");
    }
}
