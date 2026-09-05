import {renderToStaticMarkup} from 'react-dom/server';
import {describe,expect,it,vi} from 'vitest';
import {DocsPage,FeatureDetail,FeaturesPage,FindingsPage,LibraryPage,MyCodePage} from './App';

const node={id:'login',kind:'Function',name:'login',path:'src/login.ts',language:'TypeScript',start_line:10,end_line:30,source_scope:'Project'};
const feature={id:'feature:login',project_id:'project',name:'Authentication',description:'Authenticates a user.',confidence:.91,status:'accepted' as const,entry_point_node_ids:['login'],node_ids:['login'],edge_ids:[],routes:['POST /login'],pages:[],services:['AuthService'],repositories:['UserRepository'],models:[],entities:[],templates:[],tests:['login.test.ts'],external_services:[],source_hash:'abcdef'};
const spec={name:'Authentication',description:'Authenticates a user.',actors:['user'],entry_points:['POST /login'],inputs:['credentials'],outputs:['session'],business_rules:['generic failure'],operations:['verify password'],data:['User'],side_effects:['audit log'],security:['rate limit'],error_cases:['invalid credentials'],acceptance_criteria:['invalid credentials are rejected'],dependencies:['UserRepository'],provenance:'deterministic'};

describe('product knowledge views',()=>{
  it('renders Feature list actions and all Feature Detail tabs',()=>{
    const list=renderToStaticMarkup(<FeaturesPage items={[feature]} onDetect={vi.fn()} onInspect={vi.fn()} onAccept={vi.fn()} onIgnore={vi.fn()}/>);
    expect(list).toContain('Authentication');expect(list).toContain('Inspecter');expect(list).toContain('Ignorer');
    const detail=renderToStaticMarkup(<FeatureDetail feature={feature} spec={spec} snippets={[]} graph={null} findings={[]} onClose={vi.fn()} onSave={vi.fn()} onRename={vi.fn()}/>);
    for(const tab of ['Vue d’ensemble','Carte','Code source','Spécification','Tests','Qualité','Sécurité','IA','Portage'])expect(detail).toContain(`>${tab}<`);
    expect(detail).toContain('Fusionner');expect(detail).toContain('Exporter le bundle');
  });

  it('renders My Code filters and inspect/explain/impact actions',()=>{
    const html=renderToStaticMarkup(<MyCodePage items={[{node,incoming_usages:2,outgoing_dependencies:3,tests:1,complexity:4,coupling:3,reuse_score:70,knowledge_value:65,library_status:'candidate'}]} kind="all" onKind={vi.fn()} onInspect={vi.fn()} onExplain={vi.fn()} onImpact={vi.fn()} onAdd={vi.fn()}/>);
    for(const label of ['Candidats','Modifiés récemment','Sans test','Fort couplage','Voir le code','Expliquer','Impact'])expect(html).toContain(label);
  });

  it('renders finding AI controls, documentation, and both Library tabs',()=>{
    const finding={id:'finding:1',category:'security' as const,severity:'high' as const,title:'Injection candidate',description:'Evidence-based candidate.',evidence:['matched sink'],node_ids:['login'],path:'src/login.ts',start_line:10,end_line:20,detector:'taint_like_source_sink',confidence:.5,status:'open' as const};
    const findings=renderToStaticMarkup(<FindingsPage title="Security" description="Static review" items={[finding]} onInspect={vi.fn()} onStatus={vi.fn()} onExplain={vi.fn()} onReview={vi.fn()}/>);
    expect(findings).toContain('Expliquer avec l’IA');expect(findings).toContain('Lancer la revue IA');
    const docs=renderToStaticMarkup(<DocsPage projectId="project" onRefresh={vi.fn()} documentation={{project_name:'Demo',overview:'Overview',purpose:'Purpose',architecture:['Layered'],applications:[],entry_points:[],features:['Authentication'],domains:[],main_flows:[],modules:[],apis:[],data_model:[],external_services:[],workers:[],events:[],configuration:[],security_notes:[],development:[],deployment:[],known_limitations:[],content_hash:'hash',cache_hit:true,updated_at:0}}/>);
    expect(docs).toContain('Exporter en Markdown');expect(docs).toContain('Authentication');
    const library=renderToStaticMarkup(<LibraryPage entries={[]} candidates={[]} query="" generating={new Set()} errors={{}} onQuery={vi.fn()} onInspect={vi.fn()} onOpen={vi.fn()} onDelete={vi.fn()} onDocument={vi.fn()}/>);
    expect(library).toContain('Functions');expect(library).toContain('Features');
  });
});
