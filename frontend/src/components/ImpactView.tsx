import type { AtlasNode } from "../graphView";
export type ImpactResult = { source: AtlasNode; direct: AtlasNode[]; transitive: { depth: number; nodes: AtlasNode[] }[] };
export function ImpactView({ result, onNavigate }: { result: ImpactResult | null; onNavigate: (id: string) => void }) {
  if (!result) return <p>Choisissez un symbole puis lancez l’analyse d’impact.</p>;
  const groups = [{ depth: 1, nodes: result.direct }, ...result.transitive].filter(group => group.nodes.length);
  return <section className="impact-results"><h3>Impact de {result.source.name}</h3><p className="provenance">Relations entrantes du graphe actuel. Une dépendance indique un candidat à vérifier, pas une régression certaine.</p>
    {!groups.length && <p>Aucun dépendant trouvé dans la profondeur analysée.</p>}
    {groups.map(group => <details key={group.depth} open={group.depth === 1}><summary>{group.depth === 1 ? "Dépendants directs" : `Profondeur ${group.depth}`} · {group.nodes.length}</summary>{group.nodes.map(node => <button className="source-row" key={node.id} onClick={() => onNavigate(node.id)}><b>{node.name}</b><small>{node.path} · {node.kind}</small></button>)}</details>)}
  </section>;
}
