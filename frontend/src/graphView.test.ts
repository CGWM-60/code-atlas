import {describe,expect,it} from 'vitest';
import {
  MAX_VISIBLE_EDGES,
  MAX_VISIBLE_NODES,
  flowEdge,
  layoutNodes,
  visibleSubgraph,
  type AtlasEdge,
  type AtlasNode,
  type Graph,
} from './graphView';

const node=(id:string,kind:string):AtlasNode=>({id,kind,name:id});
const edge=(id:string,source_id:string,target_id:string,relation:string):AtlasEdge=>({id,source_id,target_id,relation});

function largeGraph():Graph {
  const nodes=[node('project','Project'),node('module','Module'),node('file','File')];
  const edges=[edge('contains-module','project','module','Contains'),edge('contains-file','module','file','Contains')];
  for(let index=0;index<900;index+=1){
    const id=`function-${index}`;
    nodes.push(node(id,'Function'));
    edges.push(edge(`declares-${index}`,'file',id,'Declares'));
    if(index>0)edges.push(edge(`calls-${index}`,`function-${index-1}`,id,'Calls'));
  }
  return {root:'.',nodes,edges,unresolved_calls:[]};
}

describe('graph visibility',()=>{
  it('uses the architecture projection by default and strictly caps symbol views',()=>{
    const graph=largeGraph();
    const kinds=new Set(graph.nodes.map(item=>item.kind));
    const architecture=visibleSubgraph(graph,'Architecture',kinds,new Set(),null,false);
    expect(architecture.nodes.map(item=>item.id)).toEqual(['project']);
    const symbols=visibleSubgraph(graph,'Symbols',kinds,new Set(),null,false);
    expect(symbols.nodes).toHaveLength(MAX_VISIBLE_NODES);
    expect(symbols.edges.length).toBeLessThanOrEqual(MAX_VISIBLE_EDGES);
    expect(symbols.truncated).toBe(true);
  });

  it('starts a project map at zones and groups, then lays zones left to right',()=>{
    const graph:Graph={root:'.',unresolved_calls:[],nodes:[
      node('project','Project'),
      {...node('web','ArchitectureZone'),name:'API / ROUTING'},
      {...node('domain','ArchitectureZone'),name:'DOMAIN'},
      node('web-group','ArchitectureGroup'),
      node('controller','Controller'),
    ],edges:[
      edge('project-web','project','web','Contains'),
      edge('project-domain','project','domain','Contains'),
      edge('web-group-edge','web','web-group','Contains'),
      edge('group-controller','web-group','controller','Contains'),
    ]};
    const kinds=new Set(graph.nodes.map(item=>item.kind));
    const initial=visibleSubgraph(graph,'Architecture',kinds,new Set(),null,false);
    expect(new Set(initial.nodes.map(item=>item.id))).toEqual(new Set(['project','web','domain','web-group']));
    expect(initial.nodes.some(item=>item.id==='controller')).toBe(false);
    const expanded=visibleSubgraph(graph,'Architecture',kinds,new Set(['web-group']),null,false);
    expect(expanded.nodes.some(item=>item.id==='controller')).toBe(true);
    const layout=layoutNodes(initial.nodes,initial.edges,null);
    const web=layout.find(item=>item.id==='web')!;
    const domain=layout.find(item=>item.id==='domain')!;
    expect(domain.position.x).toBeGreaterThan(web.position.x);
  });

  it('reveals children on demand and limits focus to the one-hop neighborhood',()=>{
    const graph=largeGraph();
    const kinds=new Set(graph.nodes.map(item=>item.kind));
    const expanded=visibleSubgraph(graph,'Architecture',kinds,new Set(['module']),null,false);
    expect(expanded.nodes.some(item=>item.id==='file')).toBe(true);
    const focused=visibleSubgraph(graph,'Symbols',kinds,new Set(),'function-10',true);
    expect(new Set(focused.nodes.map(item=>item.id))).toEqual(new Set(['file','function-9','function-10','function-11']));
  });

  it('keeps structural labels quiet until an edge is connected or hovered',()=>{
    const structural=edge('contains','project','module','Contains');
    expect(flowEdge(structural,null,null).label).toBeUndefined();
    expect(flowEdge(structural,'project',null).label).toBe('Contains');
    expect(flowEdge(structural,null,'contains').label).toBe('Contains');
    expect(flowEdge(structural,null,null).markerEnd).toBeDefined();
  });
});
