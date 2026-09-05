import dagre from '@dagrejs/dagre';
import {MarkerType,type Edge,type Node} from '@xyflow/react';

export type AtlasNode={id:string;kind:string;name:string;path?:string;language?:string;start_line?:number;end_line?:number;owner?:string;source_scope?:string};
export type AtlasEdge={id:string;source_id:string;target_id:string;relation:string};
export type Graph={root:string;nodes:AtlasNode[];edges:AtlasEdge[];unresolved_calls:unknown[];full_node_count?:number;full_edge_count?:number;connected_components?:number;isolated_nodes?:number;entry_points?:number;zones?:string[]};
export type ViewLevel='Architecture'|'Modules'|'Files'|'Symbols';
export type FlowNodeData=AtlasNode&{dimmed?:boolean};

export const MAX_VISIBLE_NODES=650;
export const MAX_VISIBLE_EDGES=2500;
export const VIEW_LEVELS:ViewLevel[]=['Architecture','Modules','Files','Symbols'];
const ARCHITECTURE_ROOT_KINDS=new Set(['Project','ArchitectureZone','ArchitectureGroup','EntryPoint']);
const ARCHITECTURE_KINDS=new Set([...ARCHITECTURE_ROOT_KINDS,'Workspace','Package','Crate','Module','Controller','Handler','Command','Cli','Service','Repository','Entity','Model','Worker','Job','Listener','EventSubscriber','EventListener','MessageHandler','Template','ExternalService','Infrastructure','Shared','Page','Route','ApiEndpoint','Component','Provider','Database','DatabaseTable','Event','Queue','WebSocket','Config','Environment']);
const MODULE_KINDS=new Set(['Project','Workspace','Package','Crate','Module','ArchitectureGroup','Controller','Handler','Command','Cli','Service','Repository','Entity','Model','Provider','Worker','Job','Listener','MessageHandler','Component','Page','Class','Struct','Enum','Trait','Interface']);
const FILE_KINDS=new Set([...MODULE_KINDS,'File']);
const HIERARCHY_RELATIONS=new Set(['Contains','Declares','HasMethod','EntryPoint','CONTAINS','DECLARES','HAS_METHOD','ENTRY_POINT']);
type GraphIndex={nodeById:Map<string,AtlasNode>;childrenByParent:Map<string,string[]>;incomingByTarget:Map<string,AtlasEdge[]>;outgoingBySource:Map<string,AtlasEdge[]>};
const GRAPH_INDEXES=new WeakMap<Graph,GraphIndex>();

export function graphIndex(graph:Graph):GraphIndex {
  const cached=GRAPH_INDEXES.get(graph);if(cached)return cached;
  const index:GraphIndex={nodeById:new Map(graph.nodes.map(node=>[node.id,node])),childrenByParent:new Map(),incomingByTarget:new Map(),outgoingBySource:new Map()};
  for(const edge of graph.edges){
    const incoming=index.incomingByTarget.get(edge.target_id)??[];incoming.push(edge);index.incomingByTarget.set(edge.target_id,incoming);
    const outgoing=index.outgoingBySource.get(edge.source_id)??[];outgoing.push(edge);index.outgoingBySource.set(edge.source_id,outgoing);
    if(HIERARCHY_RELATIONS.has(edge.relation)){const children=index.childrenByParent.get(edge.source_id)??[];children.push(edge.target_id);index.childrenByParent.set(edge.source_id,children)}
  }
  GRAPH_INDEXES.set(graph,index);return index;
}

export const EDGE_STYLES:Record<string,{stroke:string;width:number;dash?:string;animated?:boolean}>={
  Contains:{stroke:'#47505d',width:1},
  Declares:{stroke:'#56606e',width:1},
  HasMethod:{stroke:'#56606e',width:1},
  Calls:{stroke:'#61d095',width:2.1,animated:true},
  Uses:{stroke:'#62a8df',width:1.5},
  Imports:{stroke:'#62a8df',width:1.45,dash:'5 4'},
  Implements:{stroke:'#d58bea',width:1.9,dash:'7 3'},
  Extends:{stroke:'#f09a75',width:1.9,dash:'7 3'},
  RoutesTo:{stroke:'#ff9f43',width:2.2},
  Renders:{stroke:'#35d1ca',width:2.1},
  HandledBy:{stroke:'#ff7474',width:2.2},
};

export function kindsForLevel(level:ViewLevel):Set<string>|null {
  if(level==='Architecture')return ARCHITECTURE_ROOT_KINDS;
  if(level==='Modules')return MODULE_KINDS;
  if(level==='Files')return FILE_KINDS;
  return null;
}

export function hierarchyChildren(graph:Graph,nodeId:string):string[] {
  return graphIndex(graph).childrenByParent.get(nodeId)??[];
}

export function visibleSubgraph(graph:Graph,level:ViewLevel,enabledKinds:Set<string>,expanded:Set<string>,focusId:string|null,focusMode:boolean){
  const index=graphIndex(graph);
  const levelKinds=kindsForLevel(level);
  const ids=new Set(graph.nodes.filter(node=>enabledKinds.has(node.kind)&&(!levelKinds||levelKinds.has(node.kind))).map(node=>node.id));
  for(const parent of expanded){
    ids.add(parent);
    hierarchyChildren(graph,parent).forEach(id=>{
      const node=index.nodeById.get(id);
      if(node&&enabledKinds.has(node.kind))ids.add(id);
    });
  }
  if(focusMode&&focusId){
    ids.clear();ids.add(focusId);
    const direct=new Set<string>();
    for(const edge of index.outgoingBySource.get(focusId)??[]){ids.add(edge.target_id);direct.add(edge.target_id)}
    for(const edge of index.incomingByTarget.get(focusId)??[]){ids.add(edge.source_id);direct.add(edge.source_id)}
    // Add architecture context through semantic owners, never through a whole file.
    for(const neighborId of direct){
      const neighbor=index.nodeById.get(neighborId);
      if(!neighbor||neighbor.kind==='File'||(!ARCHITECTURE_KINDS.has(neighbor.kind)&&!MODULE_KINDS.has(neighbor.kind)))continue;
      for(const edge of index.outgoingBySource.get(neighborId)??[]){if(!HIERARCHY_RELATIONS.has(edge.relation))ids.add(edge.target_id)}
      for(const edge of index.incomingByTarget.get(neighborId)??[]){if(!HIERARCHY_RELATIONS.has(edge.relation))ids.add(edge.source_id)}
    }
  }
  const priority=(node:AtlasNode)=>node.id===focusId?-100:(ARCHITECTURE_KINDS.has(node.kind)?0:MODULE_KINDS.has(node.kind)?1:node.kind==='File'?2:3);
  const nodes=graph.nodes.filter(node=>ids.has(node.id)).sort((a,b)=>priority(a)-priority(b)||a.name.localeCompare(b.name)).slice(0,MAX_VISIBLE_NODES);
  const limitedIds=new Set(nodes.map(node=>node.id));
  const edgePriority=(edge:AtlasEdge)=>edge.source_id===focusId||edge.target_id===focusId?-10:HIERARCHY_RELATIONS.has(edge.relation)?2:0;
  const matchingEdges=[...limitedIds]
    .flatMap(id=>index.outgoingBySource.get(id)??[])
    .filter(edge=>limitedIds.has(edge.target_id));
  const edges=matchingEdges.sort((a,b)=>edgePriority(a)-edgePriority(b)).slice(0,MAX_VISIBLE_EDGES);
  return {nodes,edges,truncated:ids.size>nodes.length||matchingEdges.length>edges.length,totalCandidates:ids.size};
}

export function layoutNodes(nodes:AtlasNode[],edges:AtlasEdge[],selectedId:string|null):Node<FlowNodeData>[] {
  if(nodes.some(node=>node.kind==='ArchitectureZone'))return layoutArchitectureNodes(nodes,edges,selectedId);
  const layoutGraph=new dagre.graphlib.Graph().setDefaultEdgeLabel(()=>({}));
  layoutGraph.setGraph({rankdir:'LR',ranksep:95,nodesep:34,edgesep:18,marginx:40,marginy:40});
  nodes.forEach(node=>layoutGraph.setNode(node.id,{width:230,height:66}));
  edges.forEach(edge=>layoutGraph.setEdge(edge.source_id,edge.target_id));
  dagre.layout(layoutGraph);
  return nodes.map(node=>{
    const position=layoutGraph.node(node.id)??{x:0,y:0};
    return {id:node.id,type:'atlas',data:node,selected:node.id===selectedId,position:{x:position.x-115,y:position.y-33}};
  });
}

const ZONE_ORDER=['ENTRY POINTS','UI / FRONTEND','API / ROUTING','APPLICATION','DOMAIN','PERSISTENCE','EVENTS / QUEUES','WORKERS / JOBS','CLI','EXTERNAL SERVICES','INFRASTRUCTURE','TESTS','SHARED','UNKNOWN'];

function layoutArchitectureNodes(nodes:AtlasNode[],edges:AtlasEdge[],selectedId:string|null):Node<FlowNodeData>[] {
  const nodeById=new Map(nodes.map(node=>[node.id,node]));
  const children=new Map<string,string[]>();
  for(const edge of edges){if(edge.relation==='Contains'){const values=children.get(edge.source_id)??[];values.push(edge.target_id);children.set(edge.source_id,values)}}
  const zones=nodes.filter(node=>node.kind==='ArchitectureZone').sort((a,b)=>ZONE_ORDER.indexOf(a.name)-ZONE_ORDER.indexOf(b.name));
  const positions=new Map<string,{x:number;y:number}>();
  nodes.filter(node=>node.kind==='Project').forEach((node,index)=>positions.set(node.id,{x:-330,y:index*90}));
  nodes.filter(node=>node.kind==='EntryPoint').forEach((node,index)=>positions.set(node.id,{x:-40,y:index*90-150}));
  zones.forEach((zone,zoneIndex)=>{
    const x=300+zoneIndex*330;
    positions.set(zone.id,{x,y:0});
    const groups=(children.get(zone.id)??[]).filter(id=>nodeById.get(id)?.kind==='ArchitectureGroup');
    groups.forEach((groupId,groupIndex)=>{
      const y=110+groupIndex*105;
      positions.set(groupId,{x,y});
      (children.get(groupId)??[]).forEach((memberId,memberIndex)=>{
        positions.set(memberId,{x:x+245,y:y+memberIndex*82});
        (children.get(memberId)??[]).forEach((childId,childIndex)=>positions.set(childId,{x:x+490,y:y+childIndex*82}));
      });
    });
  });
  let overflow=0;
  return nodes.map(node=>({id:node.id,type:'atlas',data:node,selected:node.id===selectedId,position:positions.get(node.id)??{x:0,y:350+overflow++*82}}));
}

export function flowEdge(edge:AtlasEdge,selectedId:string|null,hoveredId:string|null):Edge {
  const base=EDGE_STYLES[edge.relation]??{stroke:'#77808f',width:1.35};
  const connected=Boolean(selectedId&&(edge.source_id===selectedId||edge.target_id===selectedId));
  const highlighted=connected||hoveredId===edge.id;
  const structural=HIERARCHY_RELATIONS.has(edge.relation);
  const opacity=selectedId?(connected ? .92 : .07):(structural ? .28 : .62);
  return {
    id:edge.id,
    source:edge.source_id,
    target:edge.target_id,
    type:'smoothstep',
    animated:Boolean(base.animated&&(!selectedId||connected)),
    label:highlighted?edge.relation:undefined,
    markerEnd:{type:MarkerType.ArrowClosed,color:base.stroke,width:16,height:16},
    style:{stroke:base.stroke,strokeWidth:highlighted?base.width+1:base.width,strokeDasharray:base.dash,opacity},
    labelStyle:{fill:'#d5dbe4',fontSize:9,fontWeight:600},
    labelBgStyle:{fill:'#11151d',fillOpacity:.95},
    labelBgPadding:[5,3],
    labelBgBorderRadius:4,
  };
}
