//! Diff line text in the window text selection: drag to select, Cmd+C to copy.
//! `gpui_kit::base::SelectableText` with highlights (word diff, search matches).

use gpui_kit::base::{Theme, TextSelection, TextSelectionHandle, TextSelectionRegistration, TextSelectionRun};
use gpui_kit::*;
use std::ops::Range;

pub struct CodeText {
    id: ElementId,
    text: SharedString,
    styled: StyledText,
    order: u64,
}

impl CodeText {
    /// `order` places the line among all selectable runs; copied text follows it.
    pub fn new(id: ElementId, text: SharedString, highlights: Vec<(Range<usize>, HighlightStyle)>, order: u64) -> Self {
        Self { id, styled: StyledText::new(text.clone()).with_highlights(highlights), text, order }
    }
}

impl IntoElement for CodeText {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for CodeText {
    type RequestLayoutState = TextSelectionHandle;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, TextSelectionHandle) {
        // The handle must outlive frames, or the selection resets on every repaint.
        let handle = window.with_element_state(id.expect("CodeText has an id"), |kept: Option<TextSelectionHandle>, _| {
            let handle = kept.unwrap_or_else(|| TextSelectionHandle::new(self.text.clone(), cx));
            (handle.clone(), handle)
        });
        let (layout_id, ()) = self.styled.request_layout(id, inspector_id, window, cx);
        (layout_id, handle)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        handle: &mut TextSelectionHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.styled.prepaint(id, inspector_id, bounds, &mut (), window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let registration = TextSelectionRegistration::new(hitbox, bounds)
            .with_document_order(self.order)
            .with_text_bounds(vec![bounds])
            .with_rendered_element(handle, window, cx);
        handle.register(registration, window, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        handle: &mut TextSelectionHandle,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.styled.layout().clone();
        let before = TextSelection::selected_text(window, cx);
        let run = TextSelectionRun::new(self.text.clone(), layout.clone(), bounds).with_document_order(self.order);
        let projection = handle.update_runs(&[run], cx);
        if before != TextSelection::selected_text(window, cx) {
            window.refresh();
        }
        let color = Theme::global(cx).tokens.colors.selection;
        for range in projection.ranges().iter().flatten() {
            let (Some(start), Some(end)) = (layout.position_for_index(range.start), layout.position_for_index(range.end))
            else {
                continue;
            };
            for quad in selection_quads(start, end, layout.bounds(), layout.line_height()) {
                window.paint_quad(fill(quad, color));
            }
        }
        self.styled.paint(id, inspector_id, bounds, &mut (), &mut (), window, cx);
    }
}

/// Selection rectangles from `start` to `end` over wrapped lines.
fn selection_quads(start: Point<Pixels>, end: Point<Pixels>, bounds: Bounds<Pixels>, line: Pixels) -> Vec<Bounds<Pixels>> {
    if start.y == end.y {
        return vec![Bounds::from_corners(start, point(end.x, end.y + line))];
    }
    let mut quads = vec![Bounds::from_corners(start, point(bounds.right(), start.y + line))];
    if end.y > start.y + line {
        quads.push(Bounds::from_corners(point(bounds.left(), start.y + line), point(bounds.right(), end.y)));
    }
    quads.push(Bounds::from_corners(point(bounds.left(), end.y), point(end.x, end.y + line)));
    quads
}
