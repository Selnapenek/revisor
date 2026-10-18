import ts from 'typescript';

export interface ReferenceUsage { path: string; line: number; text: string }
export interface ResolvedCommentContext { symbol: string; definition: ReferenceUsage; usages: ReferenceUsage[] }
const sourceFile = /\.(?:[cm]?[jt]s|tsx|jsx)$/i;

export function resolveCommentReferences(files: Record<string, string>, path: string, line: number): ResolvedCommentContext {
  if (!sourceFile.test(path) || files[path] === undefined) throw new Error('Only TS/JS source comments are supported.');
  const names = Object.keys(files).filter((name) => sourceFile.test(name)).map((name) => `/repo/${name}`);
  const sources = new Map(Object.entries(files).map(([name, content]) => [`/repo/${name}`, content]));
  const configs = Object.keys(files).filter((name) => /(?:^|\/)[jt]sconfig\.json$/.test(name) && (path.startsWith(name.slice(0, name.lastIndexOf('/') + 1))));
  if (configs.length > 1) throw new Error('Multiple TS/JS configs need project-aware reference resolution. No content was sent.');
  const configPath = configs[0];
  let options: ts.CompilerOptions = { allowJs: true, checkJs: false, noLib: true, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, jsx: ts.JsxEmit.Preserve };
  if (configPath) {
    const parsed = ts.parseConfigFileTextToJson(configPath, files[configPath]);
    if (parsed.error) throw new Error('The project config cannot be read.');
    if (parsed.config.extends || parsed.config.references) throw new Error('Extended or multi-project config cannot be resolved from this snapshot.');
    const converted = ts.convertCompilerOptionsFromJson(parsed.config.compilerOptions ?? {}, `/repo/${configPath.includes('/') ? configPath.slice(0, configPath.lastIndexOf('/')) : ''}`);
    if (converted.errors.length) throw new Error('The project config cannot be used for reference resolution.');
    options = { ...options, ...converted.options, noLib: true, allowJs: true };
  }
  const host: ts.LanguageServiceHost = {
    getCompilationSettings: () => options, getScriptFileNames: () => names, getScriptVersion: () => '1',
    getScriptSnapshot: (name) => { const content = sources.get(name); return content === undefined ? undefined : ts.ScriptSnapshot.fromString(content); },
    getCurrentDirectory: () => '/repo', getDefaultLibFileName: () => '/repo/__missing_lib__.d.ts',
    fileExists: (name) => sources.has(name), readFile: (name) => sources.get(name),
    directoryExists: (name) => name === '/repo' || [...sources.keys()].some((key) => key.startsWith(`${name}/`)),
    getDirectories: () => [], readDirectory: () => names,
    useCaseSensitiveFileNames: () => true, getNewLine: () => '\n',
  };
  const service = ts.createLanguageService(host);
  try {
    const program = service.getProgram();
    const moduleHost: ts.ModuleResolutionHost = { fileExists: host.fileExists!, readFile: host.readFile!, directoryExists: host.directoryExists! };
    for (const fileName of names) {
      const text = sources.get(fileName)!;
      for (const imported of ts.preProcessFile(text).importedFiles) {
        const specifier = imported.fileName;
        const local = specifier.startsWith('.') || Object.keys(options.paths ?? {}).some((pattern) => {
          const prefix = pattern.split('*')[0]; return specifier.startsWith(prefix);
        });
        if (local && !ts.resolveModuleName(specifier, fileName, options, moduleHost).resolvedModule) {
          throw new Error(`A local import cannot be resolved: ${specifier} in ${fileName.slice(6)}.`);
        }
      }
    }
    const source = program?.getSourceFile(`/repo/${path}`);
    if (!source || line < 1 || line > source.getLineAndCharacterOfPosition(source.end).line + 1) throw new Error('The comment line is not in the PR-head source.');
    const offset = source.getPositionOfLineAndCharacter(line - 1, 0);
    let chosen: { name: ts.Identifier; node: ts.Node } | undefined;
    function visit(node: ts.Node) {
      if (node.pos > offset || node.end < offset) return;
      if ((ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node) || ts.isFunctionExpression(node)) && node.name && ts.isIdentifier(node.name)) chosen = { name: node.name, node };
      if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && (ts.isArrowFunction(node.initializer) || ts.isFunctionExpression(node.initializer))) chosen = { name: node.name, node };
      ts.forEachChild(node, visit);
    }
    visit(source);
    if (!chosen) throw new Error('No named function contains this comment line.');
    const { name, node } = chosen;
    const groups = service.findReferences(source.fileName, name.getStart(source));
    if (!groups?.length) throw new Error('Resolved references are not available.');
    const snippets = new Map<string, ReferenceUsage>();
    for (const group of groups) for (const ref of group.references) {
      if (ref.isDefinition || !ref.fileName.startsWith('/repo/')) continue;
      const refSource = program?.getSourceFile(ref.fileName);
      if (!refSource) throw new Error('Reference source is missing from the snapshot.');
      const refLine = refSource.getLineAndCharacterOfPosition(ref.textSpan.start).line + 1;
      const refPath = ref.fileName.slice('/repo/'.length);
      const lines = refSource.text.split('\n');
      const text = lines.slice(Math.max(0, refLine - 3), Math.min(lines.length, refLine + 2)).map((content, index) => `${Math.max(1, refLine - 2) + index}: ${content}`).join('\n');
      snippets.set(`${refPath}:${refLine}`, { path: refPath, line: refLine, text });
    }
    return { symbol: name.text, definition: { path, line: source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1, text: node.getText(source) }, usages: [...snippets.values()].sort((a, b) => a.path.localeCompare(b.path) || a.line - b.line) };
  } finally { service.dispose(); }
}

