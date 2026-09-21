//! Module loading: `import a.b` → `a/b.oli` from the root file's directory or
//! a library directory. `core` is loaded implicitly when available.

use oli_diag::{Diagnostic, Diagnostics, SourceFile, SourceMap, Span};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where module texts come from (the file system, or memory in tests).
pub trait ModuleSource {
    /// Returns `(display name, text)` for a dotted module path such as `std.os`.
    fn load(&self, module: &str) -> Option<(String, String)>;
}

#[derive(Debug, Clone)]
pub struct FsModuleSource {
    pub root_dir: PathBuf,
    pub lib_dirs: Vec<PathBuf>,
}

impl FsModuleSource {
    /// Library directories: `$OLI_LIB`, then a `lib/` found by walking up from
    /// the executable (development layout), then `./lib`.
    pub fn default_lib_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(p) = std::env::var_os("OLI_LIB") {
            dirs.push(PathBuf::from(p));
        }
        if let Ok(exe) = std::env::current_exe() {
            let mut dir = exe.parent().map(Path::to_path_buf);
            while let Some(d) = dir {
                let candidate = d.join("lib");
                if candidate.join("core.oli").is_file() {
                    dirs.push(candidate);
                    break;
                }
                dir = d.parent().map(Path::to_path_buf);
            }
        }
        dirs.push(PathBuf::from("lib"));
        dirs
    }
}

impl ModuleSource for FsModuleSource {
    fn load(&self, module: &str) -> Option<(String, String)> {
        let rel: PathBuf = module.split('.').collect::<PathBuf>().with_extension("oli");
        let mut candidates = vec![self.root_dir.join(&rel)];
        candidates.extend(self.lib_dirs.iter().map(|d| d.join(&rel)));
        for c in candidates {
            if let Ok(text) = std::fs::read_to_string(&c) {
                return Some((c.display().to_string().replace('\\', "/"), text));
            }
        }
        None
    }
}

/// In-memory modules for tests.
#[derive(Debug, Default)]
pub struct MemoryModuleSource {
    pub modules: HashMap<String, String>,
}

impl ModuleSource for MemoryModuleSource {
    fn load(&self, module: &str) -> Option<(String, String)> {
        self.modules
            .get(module)
            .map(|t| (format!("{module}.oli"), t.clone()))
    }
}

/// A parsed module with its file id and resolved import edges.
pub(crate) struct LoadedModule {
    pub name: String,
    pub file: u32,
    pub ast: oli_ast::Module,
    /// (alias, target module index if it loaded, span of the import line)
    pub imports: Vec<(String, Option<usize>, Span)>,
}

/// Loads the root and, transitively, every imported module. Index 0 is the root.
pub(crate) fn load_all(
    root: SourceFile,
    source: &dyn ModuleSource,
    sources: &mut SourceMap,
    diags: &mut Diagnostics,
) -> Vec<LoadedModule> {
    let mut loaded: Vec<LoadedModule> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    // (module path, who asked: (file, span) or None for the implicit `core`)
    let mut queue: Vec<(String, Option<(u32, Span)>)> = Vec::new();

    let root_file = sources.add(root);
    let root_ast = parse_into(root_file, sources, diags);
    let root_name = root_ast
        .name
        .as_ref()
        .map(oli_ast::Path::dotted)
        .unwrap_or_else(|| {
            sources
                .get(root_file)
                .map_or("main".to_string(), root_module_name)
        });
    push_module(
        &mut loaded,
        &mut by_name,
        root_name,
        root_file,
        root_ast,
        &mut queue,
    );
    queue.push(("core".to_string(), None));

    while let Some((name, requester)) = queue.pop() {
        if by_name.contains_key(&name) {
            continue;
        }
        match source.load(&name) {
            Some((display, text)) => {
                let file = sources.add(SourceFile::new(display, text));
                let ast = parse_into(file, sources, diags);
                push_module(&mut loaded, &mut by_name, name, file, ast, &mut queue);
            }
            None => {
                if let Some((file, span)) = requester {
                    diags.push(
                        Diagnostic::error("E0104", format!("cannot find module `{name}`"), span)
                            .with_note("modules are looked up as `a/b.oli` next to the root file and in the library directories")
                            .in_file(file),
                    );
                }
            }
        }
    }
    // Resolve import edges now that everything that exists is loaded.
    for m in &mut loaded {
        let paths: Vec<String> = m.ast.imports.iter().map(|imp| imp.path.dotted()).collect();
        for (k, path) in paths.iter().enumerate() {
            let target = by_name.get(path).copied();
            if let Some(edge) = m.imports.get_mut(k) {
                edge.1 = target;
            }
        }
    }
    loaded
}

fn root_module_name(file: &SourceFile) -> String {
    Path::new(file.name())
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string()
}

fn parse_into(file: u32, sources: &SourceMap, diags: &mut Diagnostics) -> oli_ast::Module {
    let mut local = Diagnostics::new();
    let ast = match sources.get(file) {
        Some(f) => oli_parser::parse_source(f, &mut local),
        None => oli_ast::Module {
            name: None,
            imports: Vec::new(),
            decls: Vec::new(),
        },
    };
    diags.absorb(local, file);
    ast
}

fn push_module(
    loaded: &mut Vec<LoadedModule>,
    by_name: &mut HashMap<String, usize>,
    name: String,
    file: u32,
    ast: oli_ast::Module,
    queue: &mut Vec<(String, Option<(u32, Span)>)>,
) {
    let mut imports = Vec::new();
    for imp in &ast.imports {
        let alias = imp
            .alias
            .as_ref()
            .map(|a| a.name.clone())
            .or_else(|| imp.path.segments.last().map(|s| s.name.clone()))
            .unwrap_or_default();
        queue.push((imp.path.dotted(), Some((file, imp.span))));
        imports.push((alias, None, imp.span));
    }
    by_name.insert(name.clone(), loaded.len());
    loaded.push(LoadedModule {
        name,
        file,
        ast,
        imports,
    });
}
