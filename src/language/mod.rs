pub mod detector;

use serde::{Deserialize, Serialize};

/// Représente le langage ou le type de fichier
/// détecté par notre moteur d'analyse.
///
/// Pourquoi créer un enum au lieu d'utiliser simplement String ?
///
/// On pourrait écrire :
///
///     language: String
///
/// et mettre dedans "Rust", "PHP", "Python", etc.
///
/// Mais une String permettrait aussi accidentellement :
///
///     "Ruts"
///     "Pythno"
///     "nimporte_quoi"
///
/// Avec un enum, Rust nous oblige à utiliser uniquement
/// les valeurs que nous avons prévues.
///
/// Cela rend notre programme beaucoup plus sûr.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProgrammingLanguage {
    Rust,
    Dart,

    JavaScript,
    TypeScript,

    Php,
    Python,

    Go,

    Java,
    Kotlin,

    C,
    Cpp,
    COrCppHeader,

    CSharp,

    Swift,

    Html,
    Css,

    Sql,

    Json,
    Toml,
    Yaml,

    Markdown,

    Shell,

    Dockerfile,
    Makefile,

    /// Cette valeur est utilisée lorsque notre programme
    /// ne connaît pas encore le type du fichier.
    ///
    /// Exemple :
    ///
    ///     fichier.xyz
    ///
    /// Si ".xyz" n'est pas connu, on retourne Unknown.
    Unknown,
}

impl ProgrammingLanguage {
    /// Retourne le nom lisible du langage.
    ///
    /// Exemple :
    ///
    ///     ProgrammingLanguage::Rust.as_str()
    ///
    /// retourne :
    ///
    ///     "Rust"
    ///
    /// Le `&self` signifie :
    /// "la fonction utilise l'objet actuel sans le modifier".
    ///
    /// `&'static str` signifie que nous retournons une chaîne
    /// de caractères constante qui existe pendant toute
    /// la durée du programme.
    pub fn as_str(&self) -> &'static str {
        match self {
            ProgrammingLanguage::Rust => "Rust",

            ProgrammingLanguage::Dart => "Dart",

            ProgrammingLanguage::JavaScript => "JavaScript",
            ProgrammingLanguage::TypeScript => "TypeScript",

            ProgrammingLanguage::Php => "PHP",
            ProgrammingLanguage::Python => "Python",

            ProgrammingLanguage::Go => "Go",

            ProgrammingLanguage::Java => "Java",
            ProgrammingLanguage::Kotlin => "Kotlin",

            ProgrammingLanguage::C => "C",
            ProgrammingLanguage::Cpp => "C++",
            ProgrammingLanguage::COrCppHeader => "C/C++ Header",

            ProgrammingLanguage::CSharp => "C#",

            ProgrammingLanguage::Swift => "Swift",

            ProgrammingLanguage::Html => "HTML",
            ProgrammingLanguage::Css => "CSS",

            ProgrammingLanguage::Sql => "SQL",

            ProgrammingLanguage::Json => "JSON",
            ProgrammingLanguage::Toml => "TOML",
            ProgrammingLanguage::Yaml => "YAML",

            ProgrammingLanguage::Markdown => "Markdown",

            ProgrammingLanguage::Shell => "Shell",

            ProgrammingLanguage::Dockerfile => "Dockerfile",
            ProgrammingLanguage::Makefile => "Makefile",

            ProgrammingLanguage::Unknown => "Unknown",
        }
    }
}
