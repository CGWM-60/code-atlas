import { useId } from "react";
import Markdown from "react-markdown";

const labels: Record<string, string> = {
  name: "Nom", description: "Description", purpose: "Objectif", actors: "Acteurs",
  entry_points: "Points d’entrée", flow: "Flux", inputs: "Entrées", outputs: "Sorties",
  business_rules: "Règles métier", operations: "Opérations", data: "Données",
  side_effects: "Effets de bord", security: "Sécurité", error_cases: "Cas d’erreur",
  acceptance_criteria: "Critères d’acceptation", dependencies: "Dépendances", tests: "Tests",
  known_limitations: "Limites connues", provenance: "Provenance", feature_name: "Feature",
  target_profile: "Profil cible", files_to_create: "Fichiers à créer", files_to_modify: "Fichiers à modifier",
  security_requirements: "Exigences de sécurité", constraints: "Contraintes", risks: "Risques",
  target_conventions: "Conventions du projet cible", validation_commands: "Commandes de validation",
};

/** Domain sections shared by Feature specifications, documentation and porting plans. */
export function KnowledgeDocument({ value }: { value: Record<string, unknown> }) {
  const prefix = useId();
  const sections = Object.entries(value).filter(([key]) => key in labels);
  return <article className="knowledge-document">
    <nav className="docs-toc" aria-label="Sections du document">{sections.map(([key]) => <a key={key} href={`#${prefix}-${key}`}>{labels[key]}</a>)}</nav>
    {sections.map(([key, content]) => <section key={key} id={`${prefix}-${key}`}>
      <h3>{labels[key]}</h3>
      {Array.isArray(content)
        ? content.length ? <ul>{content.filter(item => typeof item === "string").map((item, i) => <li key={i}><Markdown skipHtml>{item}</Markdown></li>)}</ul> : <p className="provenance">Aucun élément renseigné.</p>
        : typeof content === "string" ? <Markdown skipHtml>{content}</Markdown> : <p>Non renseigné.</p>}
    </section>)}
  </article>;
}
