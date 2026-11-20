"use strict";
const POSTMAN_SCHEMA = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";
const SECRET_NAME = /^(authorization|proxy-authorization|cookie|set-cookie|x-api-key|.*api[_-]?key.*|.*token.*|.*secret.*|.*password.*)$/i;
function pairsFromUrl(url) {
  const raw = typeof url === "string" ? url : url?.raw;
  if (typeof raw !== "string" || !raw.trim()) throw new Error("Collection request is missing a URL.");
  let parsed;
  try { parsed = new URL(raw.replace(/\{\{[^}]+\}\}/g, "thor-placeholder")); }
  catch { throw new Error("Collection request has an invalid URL."); }
  const query = typeof url === "object" && Array.isArray(url.query) ? url.query.filter(x => !x.disabled).map(x => ({ name: String(x.key), value: String(x.value ?? "") })) : [...parsed.searchParams].map(([name,value]) => ({name,value}));
  return { url: raw.split("?")[0], query };
}
function parseCollection(collection) {
  if (collection?.info?.schema !== POSTMAN_SCHEMA) throw new Error("Import a Postman Collection v2.1 JSON file.");
  if (!Array.isArray(collection.item) || collection.item.length > 1000) throw new Error("Collection must contain at most 1,000 request items.");
  const requests = [];
  function visit(items, prefix, depth) {
    if (depth > 12) throw new Error("Collection folder nesting exceeds 12 levels.");
    for (const item of items) {
      if (item?.request) {
        const request = item.request;
        if (typeof request === "string") throw new Error("Imported cURL strings are not supported.");
        const {url,query}=pairsFromUrl(request.url);
        const headers=(request.header||[]).filter(h=>!h.disabled).map(h=>({name:String(h.key),value:String(h.value??"")}));
        const auth=request.auth;
        const containsLiteralSecret=(name,value)=>SECRET_NAME.test(name)&&Boolean(value)&&!/^\{\{[^}]+\}\}$/.test(value.trim());
        if(headers.some(h=>containsLiteralSecret(h.name,h.value))||query.some(h=>containsLiteralSecret(h.name,h.value))) throw new Error("Replace literal credentials with {{variable}} references before importing.");
        if (auth && auth.type && auth.type !== "noauth") {
          const values=auth[auth.type];
          if (Array.isArray(values) && values.some(x=>SECRET_NAME.test(x.key||"")&&!/^\{\{[^}]+\}\}$/.test(x.value||""))) throw new Error("Replace literal credentials with {{variable}} references before importing.");
        }
        const body=request.body;
        let normalizedBody={type:"empty"};
        if(body&&body.mode==="raw"&&body.raw){
          const json=(body.options?.raw?.language==="json");
          normalizedBody=json?{type:"json",value:JSON.parse(body.raw)}:{type:"text",text:body.raw,content_type:"text/plain"};
        } else if(body&&body.mode==="urlencoded") normalizedBody={type:"form",entries:(body.urlencoded||[]).filter(x=>!x.disabled).map(x=>({name:String(x.key),value:String(x.value??"")}))};
        else if(body&&body.mode&&body.mode!=="none") throw new Error(`Unsupported request body mode: ${body.mode}`);
        const checkBody=(value)=>{if(Array.isArray(value))return value.some(checkBody);if(value&&typeof value==="object")return Object.entries(value).some(([key,item])=>containsLiteralSecret(key,String(item??""))||checkBody(item));return false;};
        if(checkBody(normalizedBody)) throw new Error("Replace literal credentials with {{variable}} references before importing.");
        const title=[...prefix,item.name||"Request"].filter(Boolean).join(" / ");
        requests.push({title,request:{method:String(request.method||"GET").toUpperCase(),url,headers,query,body:normalizedBody,auth:{type:"none"}}});
        if(requests.length>1000) throw new Error("Collection contains more than 1,000 requests.");
      } else if(Array.isArray(item?.item)) visit(item.item,[...prefix,item.name||"Folder"],depth+1);
    }
  }
  visit(collection.item,[],0);
  return {name:String(collection.info.name||"Imported collection").slice(0,160),requests};
}
function redactPair(pair){return {name:pair.name,value:SECRET_NAME.test(pair.name)?"{{redacted_secret}}":pair.value};}
function redactJson(value){if(Array.isArray(value))return value.map(redactJson);if(value&&typeof value==="object")return Object.fromEntries(Object.entries(value).map(([key,item])=>[key,SECRET_NAME.test(key)?"{{redacted_secret}}":redactJson(item)]));return value;}
function exportCollection(name, documents) {
  return {info:{name:String(name||"Thor collection").slice(0,160),schema:POSTMAN_SCHEMA},item:documents.map(doc=>{
    const req=doc.request;
    const url=new URL(req.url.replace(/\{\{[^}]+\}\}/g,"thor-placeholder"));
    url.username="";url.password="";
    for(const q of req.query||[]) url.searchParams.append(q.name,SECRET_NAME.test(q.name)?"{{redacted_secret}}":q.value);
    for(const [key] of [...url.searchParams])if(SECRET_NAME.test(key)){url.searchParams.set(key,"{{redacted_secret}}");}
    const raw=url.toString().replace(/thor-placeholder/g,"{{variable}}");
    const item={name:doc.title,request:{method:req.method,url:raw,header:(req.headers||[]).map(h=>{const safe=redactPair({name:h.name,value:h.value});return {key:safe.name,value:safe.value};}),auth:{type:"noauth"}}};
    if(req.body?.type==="json") item.request.body={mode:"raw",raw:JSON.stringify(redactJson(req.body.value),null,2),options:{raw:{language:"json"}}};
    else if(req.body?.type==="text") {let text=req.body.text;try{text=JSON.stringify(redactJson(JSON.parse(text)),null,2);}catch{text=text.replace(/((?:token|secret|password|api[_-]?key)\s*[=:]\s*)[^\s&,;]+/ig,"$1{{redacted_secret}}");}item.request.body={mode:"raw",raw:text};}
    else if(req.body?.type==="form") item.request.body={mode:"urlencoded",urlencoded:req.body.entries.map(e=>{const safe=redactPair(e);return {key:safe.name,value:safe.value};})};
    return item;
  })};
}
function evaluateResponseChecks(response, source){
 const spec=typeof source==="string"?JSON.parse(source):source;
 if(!spec||Array.isArray(spec)||typeof spec!=="object"||Object.keys(spec).some(k=>!["status","headers","json"].includes(k)))throw new Error("Checks may contain only status, headers, and json.");
 const checks=[];if(spec.status!==undefined){if(!Number.isInteger(spec.status))throw new Error("Expected status must be an integer.");checks.push({name:`HTTP status is ${spec.status}`,passed:response.status===spec.status});}
 if(spec.headers!==undefined){if(!spec.headers||Array.isArray(spec.headers)||typeof spec.headers!=="object"||Object.values(spec.headers).some(v=>typeof v!=="string"))throw new Error("Expected headers must be a JSON object of strings.");for(const [name,value] of Object.entries(spec.headers)){const actual=(response.headers||[]).find(h=>h.name.toLowerCase()===name.toLowerCase())?.value;checks.push({name:`Header ${name} matches`,passed:actual===value});}}
 if(spec.json!==undefined){if(response.body?.encoding!=="utf8")throw new Error("JSON checks require a UTF-8 JSON response.");let body;try{body=JSON.parse(response.body.data)}catch{throw new Error("Response body is not valid JSON.")}const visit=(expected,path)=>{for(const [key,value]of Object.entries(expected)){const actual=body?.[key];const label=path?`${path}.${key}`:key;if(value&&typeof value==="object"&&!Array.isArray(value))visitNested(actual,value,label);else checks.push({name:`JSON ${label} matches`,passed:JSON.stringify(actual)===JSON.stringify(value)});}};const visitNested=(actual,expected,path)=>{for(const [key,value]of Object.entries(expected)){const child=actual?.[key],label=`${path}.${key}`;if(value&&typeof value==="object"&&!Array.isArray(value))visitNested(child,value,label);else checks.push({name:`JSON ${label} matches`,passed:JSON.stringify(child)===JSON.stringify(value)});}};visit(spec.json,"");}
 return checks;
}
function parseEnvironment(environment){
 if(!environment||!Array.isArray(environment.values)||environment.values.length>1000)throw new Error("Import a Postman environment with at most 1,000 variables.");
 const values={};for(const item of environment.values){if(item.enabled===false)continue;if(!/^[A-Za-z_][A-Za-z0-9_.-]{0,127}$/.test(item.key||""))throw new Error("Environment contains an invalid variable name.");if(typeof item.value!=="string")throw new Error("Environment values must be strings.");values[item.key]=item.value;}
 return {name:String(environment.name||"Imported environment").slice(0,120),values};
}
function exportEnvironment(name,values){
 return {id:"thor-local-environment",name:String(name||"Thor environment").slice(0,120),values:Object.entries(values).map(([key,value])=>({key,value:SECRET_NAME.test(key)?"":String(value),enabled:true,type:SECRET_NAME.test(key)?"secret":"default"})),_postman_variable_scope:"environment",_postman_exported_using:"Thor"};
}
const api={parseCollection,exportCollection,parseEnvironment,exportEnvironment,evaluateResponseChecks,POSTMAN_SCHEMA};
if(typeof module!=="undefined") module.exports=api;
if(typeof window!=="undefined") window.ThorCollections=api;
