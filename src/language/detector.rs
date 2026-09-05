use crate::language::ProgrammingLanguage;
use std::path::Path;

/*
pub veux dire que l'on peux utiliser sur d autre modules
fn est la fonction
ensuite le nom de la fonction
path est le nom de la variable
Path et le type & est la reference
->ProgammingLanguage fournis le type retourner


*/
pub fn detect_language(path: &Path) -> ProgrammingLanguage {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();

    match file_name.as_str() {
        "dockerfile" => {
            return ProgrammingLanguage::Dockerfile;
        }

        "makefile" => {
            return ProgrammingLanguage::Makefile;
        }
        _ => {}
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_lowercase();
    match extension.as_str() {
        "rs" => ProgrammingLanguage::Rust,

        "dart" => ProgrammingLanguage::Dart,

        // Le symbole `|` signifie "OU".
        //
        // Donc :
        //
        // js OU jsx OU mjs OU cjs
        //
        // seront considérés comme du JavaScript.
        "js" | "jsx" | "mjs" | "cjs" => ProgrammingLanguage::JavaScript,

        "ts" | "tsx" | "mts" | "cts" => ProgrammingLanguage::TypeScript,

        "php" => ProgrammingLanguage::Php,

        "py" => ProgrammingLanguage::Python,

        "go" => ProgrammingLanguage::Go,

        "java" => ProgrammingLanguage::Java,

        "kt" | "kts" => ProgrammingLanguage::Kotlin,

        "c" => ProgrammingLanguage::C,

        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => ProgrammingLanguage::Cpp,

        // Un fichier .h peut être utilisé aussi bien
        // en C qu'en C++.
        //
        // Il serait donc incorrect de dire automatiquement
        // qu'il s'agit forcément de C.
        "h" => ProgrammingLanguage::COrCppHeader,

        "cs" => ProgrammingLanguage::CSharp,

        "swift" => ProgrammingLanguage::Swift,

        "html" | "htm" => ProgrammingLanguage::Html,

        "css" | "scss" | "sass" => ProgrammingLanguage::Css,

        "sql" => ProgrammingLanguage::Sql,

        "json" => ProgrammingLanguage::Json,

        "toml" => ProgrammingLanguage::Toml,

        "yaml" | "yml" => ProgrammingLanguage::Yaml,

        "md" | "markdown" => ProgrammingLanguage::Markdown,

        "sh" | "bash" | "zsh" => ProgrammingLanguage::Shell,

        // Si aucun cas précédent ne correspond,
        // nous retournons Unknown.
        //
        // Cela permet au scanner de continuer
        // même lorsqu'il rencontre un type de fichier
        // qu'il ne connaît pas encore.
        _ => ProgrammingLanguage::Unknown,
    }
}
