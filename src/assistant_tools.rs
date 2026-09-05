//! One internal tool vocabulary. Existing MCP/domain implementations remain authoritative.
use crate::{flow::{FlowDirection,FlowTraceRequest,trace_flow},storage::Repository};
use anyhow::{anyhow,bail,Result};
use serde_json::{json,Map,Value};

pub const TOOLS: &[(&str,&str)] = &[
 ("search_code","atlas_search"),("semantic_search","atlas_semantic_search"),("search_symbols","atlas_search"),
 ("get_node","atlas_get_node"),("get_source","atlas_get_source"),("get_callers","atlas_get_callers"),("get_callees","atlas_get_callees"),("get_dependencies","atlas_get_dependencies"),("get_impact","atlas_get_impact"),
 ("list_features","atlas_list_features"),("get_feature","atlas_get_feature"),("get_feature_graph","atlas_get_feature_graph"),("get_feature_source","atlas_get_feature_code"),("find_similar_features","atlas_find_similar_feature"),
 ("get_security_findings","atlas_get_security_findings"),("get_quality_findings","atlas_get_quality_findings"),("explain_finding",""),("trace_flow",""),
 ("list_api_endpoints","atlas_list_api_endpoints"),("get_api_endpoint","atlas_get_api_endpoint"),("get_api_contract","atlas_get_api_contract"),
 ("get_library_entries",""),("get_project_docs","atlas_get_project_docs"),("get_context","atlas_get_context"),("get_git_diff","atlas_get_git_diff"),("estimate_change","atlas_estimate_change"),("generate_test_plan","atlas_get_test_plan")
];
pub fn execute(repo:&Repository,project:&str,name:&str,mut args:Map<String,Value>)->Result<Value> {
    let (_,mcp)=TOOLS.iter().find(|(tool,_)|*tool==name).ok_or_else(||anyhow!("Unknown assistant tool: {name}"))?;
    args.insert("project_id".into(),json!(project)); // UI/provider cannot cross the active project boundary.
    if name=="get_library_entries" {return Ok(json!(repo.list_library(args.get("query").and_then(Value::as_str))?.into_iter().filter(|entry|entry.source_project_id==project).collect::<Vec<_>>()));}
    if name=="explain_finding" {
        let id=args.get("finding_id").and_then(Value::as_str).ok_or_else(||anyhow!("finding_id required"))?;
        let finding=repo.get_finding(id)?.filter(|f|f.project_id==project).ok_or_else(||anyhow!("finding not found in active project"))?;
        return Ok(json!({"finding":finding,"provenance":"Persisted detector evidence. A finding is a candidate, not a confirmed exploit."}));
    }
    if name=="trace_flow" {
        let graph=repo.load(project)?.ok_or_else(||anyhow!("project not found"))?;
        let start=args.get("node_id").or_else(||args.get("start_node_id")).and_then(Value::as_str).ok_or_else(||anyhow!("node_id required"))?;
        if graph.find_node(start).is_none(){bail!("node not found");}
        let end=args.get("end_node_id").and_then(Value::as_str).map(str::to_owned);
        return Ok(json!(trace_flow(&graph,&FlowTraceRequest{start_node_id:start.into(),end_node_id:end.clone(),direction:if end.is_some(){FlowDirection::Between}else{FlowDirection::Downstream},max_depth:args.get("depth").and_then(Value::as_u64).unwrap_or(6).min(10) as usize,relations:vec![]})));
    }
    if name=="find_similar_features" {args.insert("limit".into(),json!(10));}
    crate::mcp::call_tool(repo,mcp,&args)
}
