use std::{collections::{BTreeMap, BTreeSet}, env, fs, path::PathBuf};
use proc_macro2::{Span, LineColumn, TokenStream, TokenTree, Group};
use serde_json::Value;
use syn::{visit::{self, Visit}, spanned::Spanned, Expr, Pat, Stmt, parse::Parser};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Range(usize, usize);
#[derive(Clone)]
struct Node { range: Range, kind: &'static str, inner: Option<Range>, right: Option<Range> }
struct Local { name: String, start: usize, init: Range, scope: Range }
struct Wrapper { name: String, range: Range }
struct Ast<'a> { source: &'a str, lines: Vec<usize>, nodes: Vec<Node>, locals: Vec<Local>, wrappers: Vec<Wrapper>, comparisons: Vec<(Range,Range)>, blocks: Vec<Range> }
impl<'a> Ast<'a> {
    fn new(source: &'a str) -> Self { let mut lines = vec![0]; lines.extend(source.bytes().enumerate().filter_map(|(n,b)| (b==b'\n').then_some(n+1))); Self { source, lines, nodes:vec![], locals:vec![], wrappers:vec![], comparisons:vec![], blocks:vec![] } }
    fn offset(&self, lc: LineColumn) -> usize { let start=self.lines[lc.line-1]; start+self.source[start..].split('\n').next().unwrap().char_indices().map(|(n,_)|n).chain(std::iter::once(self.source[start..].split('\n').next().unwrap().len())).nth(lc.column).unwrap_or(lc.column) }
    fn range(&self, span: Span) -> Range { let mut start=self.offset(span.start());if start>0 && self.source.as_bytes()[start-1]==b'$'{start-=1} Range(start,self.offset(span.end())) }
    fn text(&self,r:Range)->&str { &self.source[r.0..r.1] }
    fn local(&self,name:&str,at:Range)->Option<&Local>{self.locals.iter().filter(|local|local.name==name && local.start<at.0 && local.scope.0<=at.0 && at.1<=local.scope.1).max_by_key(|local|local.start)}
    fn templates(&mut self,tokens:TokenStream){
        for token in tokens {
            if let TokenTree::Group(group)=token {
                let clean=clean(group.stream());
                if let Ok(file)=syn::parse2::<syn::File>(clean.clone()){self.visit_file(&file);}
                else if let Ok(block)=syn::parse2::<syn::Block>(TokenTree::Group({let mut g=Group::new(proc_macro2::Delimiter::Brace,clean);g.set_span(group.span());g}).into()){self.visit_block(&block);}
                else {self.templates(group.stream());}
            }
        }
    }
}
fn clean(tokens:TokenStream)->TokenStream{
    let mut out=TokenStream::new();let mut it=tokens.into_iter().peekable();while let Some(token)=it.next(){match token {
        TokenTree::Punct(ref p) if p.as_char()=='$' && matches!(it.peek(),Some(TokenTree::Ident(_)))=>{},
        TokenTree::Group(g)=>{let mut n=Group::new(g.delimiter(),clean(g.stream()));n.set_span(g.span());out.extend([TokenTree::Group(n)]);},
        t=>out.extend([t]),
    }}out
}
fn ident(pat:&Pat)->Option<String> { match pat {Pat::Ident(p)=>Some(p.ident.to_string()),Pat::Type(p)=>ident(&p.pat),_=>None} }
fn path(expr:&Expr)->Option<String>{ match expr {Expr::Path(p) if p.path.segments.len()==1=>Some(p.path.segments[0].ident.to_string()),_=>None} }
fn wrapper(expr:&Expr)->Option<String> {
    let Expr::Paren(p)=expr else{return None};
    let Expr::Block(b)=&*p.expr else{return None};
    let [Stmt::Local(l),Stmt::Expr(last,None)]=b.block.stmts.as_slice() else{return None};
    let Pat::TupleStruct(pat)=&l.pat else{return None};
    if !pat.path.is_ident("Ok") || pat.elems.len()!=1{return None}
    let binding=ident(&pat.elems[0])?;
    if path(last)?.as_str()!=binding{return None}
    path(&l.init.as_ref()?.expr)
}
impl<'ast> Visit<'ast> for Ast<'_> {
    fn visit_block(&mut self,block:&'ast syn::Block){self.blocks.push(self.range(block.span()));visit::visit_block(self,block);self.blocks.pop();}
    fn visit_local(&mut self,l:&'ast syn::Local){
        if let (Some(name),Some(init),Some(scope))=(ident(&l.pat),&l.init,self.blocks.last()){self.locals.push(Local{name,start:self.range(l.span()).0,init:self.range(init.expr.span()),scope:*scope});}
        visit::visit_local(self,l);
    }
    fn visit_expr(&mut self,e:&'ast Expr){
        let (kind,inner)=match e {
            Expr::MethodCall(e)=>("method",Some(self.range(e.receiver.span()))),
            Expr::Field(e)=>("field",Some(self.range(e.base.span()))),
            Expr::Index(e)=>("index",Some(self.range(e.expr.span()))),
            Expr::Cast(e)=>("cast",Some(self.range(e.expr.span()))),
            Expr::Unary(e)=>("unary",Some(self.range(e.expr.span()))),
            Expr::Call(_)=>("call",None),
            Expr::Binary(e)=>("binary",Some(self.range(e.left.span()))),
            Expr::Path(_)=>("path",None),
            _=>("other",None),
        };
        let right=match e {Expr::Binary(e)=>Some(self.range(e.right.span())),_=>None};
        self.nodes.push(Node{range:self.range(e.span()),kind,inner,right});
        if let Some(name)=wrapper(e){self.wrappers.push(Wrapper{name,range:self.range(e.span())});}
        visit::visit_expr(self,e);
    }
    fn visit_macro(&mut self,m:&'ast syn::Macro){
        let parser=syn::punctuated::Punctuated::<Expr,syn::Token![,]>::parse_terminated;
        if let Ok(args)=parser.parse2(clean(m.tokens.clone())){
            if (m.path.is_ident("assert_eq") || m.path.is_ident("assert_ne")) && args.len()>=2 {self.comparisons.push((self.range(args[0].span()),self.range(args[1].span())));}
            for arg in args {self.visit_expr(&arg);}
        }else{self.templates(m.tokens.clone());}
    }
}
fn range(span:&Value)->Option<Range>{Some(Range(span["byte_start"].as_u64()? as usize,span["byte_end"].as_u64()? as usize))}
fn never(label:&str)->bool{label.contains("Result<") && label.contains("Never")}
fn extract(s:&str)->String{format!("({{ let Ok(explicit_value) = {s}; explicit_value }})")}
fn validate(span:&Value,source:&str)->bool {
    let Some(r)=range(span) else{return false};
    if r.1>source.len() || !source.is_char_boundary(r.0) || !source.is_char_boundary(r.1){return false}
    let Some(line)=span["line_start"].as_u64() else{return false};
    let Some(column)=span["column_start"].as_u64() else{return false};
    let Some(current)=source.lines().nth(line as usize-1)else{return false};
    let prefix:usize=source.split_inclusive('\n').take(line as usize-1).map(str::len).sum();
    let Some(offset)=current.char_indices().map(|(n,_)|n).chain(std::iter::once(current.len())).nth(column as usize-1)else{return false};
    prefix+offset==r.0 && Some(current)==span["text"][0]["text"].as_str()
}
fn repair(source:&str, messages:&[Value])->BTreeMap<Range,String>{
    let tree=syn::parse_file(source).expect("parse source"); let mut ast=Ast::new(source);ast.visit_file(&tree);
    let mut edits=BTreeMap::new(); let mut normalized=BTreeSet::new();
    for w in &ast.wrappers {
        if let Some(l)=ast.local(&w.name,w.range){
            if ast.text(l.init).contains("let Ok("){continue}
            edits.insert(l.init,extract(ast.text(l.init)));
            edits.insert(w.range,w.name.clone()); normalized.insert(w.name.clone());
        }
    }
    for message in messages {
        let code=message["code"]["code"].as_str().unwrap_or("");
        for span in message["spans"].as_array().into_iter().flatten(){
            if !validate(span,source){continue}
            let r=range(span).unwrap(); let label=span["label"].as_str().unwrap_or("");
            if matches!(code,"E0308"|"E0271") && label.contains("found `Result<") && label.contains("Never") {
                if let Some((expected,found))=label.split_once(", found ") {if !expected.contains("Never") && never(found){edits.insert(r,extract(ast.text(r)));}}
            }
            if code=="E0308" && label.starts_with("expected `Result<") && label.contains("Never") && !label.split(", found ").nth(1).is_some_and(never){
                if let Some((left,right))=ast.comparisons.iter().find(|(left,right)|*left==r || *right==r){let other=if *left==r{*right}else{*left};edits.insert(other,extract(ast.text(other)));}
            }
            if code=="E0369" && never(label) && !label.contains("expected") {
                let text=ast.text(r);
                if let Some(local)=ast.local(text,r) {
                    if !ast.text(local.init).contains("let Ok("){edits.insert(local.init,extract(ast.text(local.init)));}
                }else{edits.insert(r,extract(text));}
            }
            if !span["is_primary"].as_bool().unwrap_or(false){continue}
            let rendered=message["rendered"].as_str().unwrap_or("");
            let title=message["message"].as_str().unwrap_or("");
            let needed=match code {"E0599" if never(rendered) && (title.contains("`Result<") || title.contains("`std::result::Result<"))=>Some("method"),"E0609" if never(rendered)=>Some("field"),"E0608" if never(rendered)=>Some("index"),"E0605" if never(rendered)=>Some("cast"),"E0600"|"E0614" if never(title)=>Some("unary"),_=>None};
            if code=="E0277" && never(title) && (title.contains("bound `Result<") || title.contains("bound `std::result::Result<") || title.contains("cannot be indexed by `Result<") || title.contains("cannot be indexed by `std::result::Result<")) {
                if ast.nodes.iter().any(|node|node.range==r && matches!(node.kind,"call"|"method"|"path")) {
                    let text=ast.text(r);
                    if let Some(local)=ast.local(text,r){
                        if !ast.text(local.init).contains("let Ok("){edits.insert(local.init,extract(ast.text(local.init)));}
                    }else{edits.insert(r,extract(text));}
                }
            }
            if code=="E0277" && never(label) && label.starts_with("no implementation for `") {
                if let Some(binary)=ast.nodes.iter().filter(|node|node.kind=="binary" && node.range.0<=r.0 && r.1<=node.range.1).min_by_key(|node|node.range.1-node.range.0){
                    for operator in [" + "," - "," * "," / "," % "," << "," >> "," & "," | "," ^ "] {
                        if let Some((left,right))=label.split_once(operator){
                            for (ty,operand) in [(left,binary.inner.unwrap()),(right,binary.right.unwrap())]{
                                if !never(ty){continue}let text=ast.text(operand);
                                if let Some(local)=ast.local(text,operand){if !ast.text(local.init).contains("let Ok("){edits.insert(local.init,extract(ast.text(local.init)));}}else{edits.insert(operand,extract(text));}
                            }
                            break
                        }
                    }
                }
            }
            if let Some(kind)=needed {
                if let Some(n)=ast.nodes.iter().filter(|n|n.kind==kind && n.range.0<=r.0 && n.range.1>=r.1).min_by_key(|n|n.range.1-n.range.0){
                    let inner=n.inner.unwrap(); let text=ast.text(inner);
                    if let Some(local)=ast.local(text,inner){
                        if !normalized.contains(text){edits.insert(local.init,extract(ast.text(local.init)));}
                    }else{edits.insert(inner,extract(text));}
                }
            }
        }
    }
    edits
}
fn main(){
    let mut args=env::args().skip(1);let diagnostics=args.next().expect("diagnostic JSON path");let mut scopes:Vec<String>=args.collect();let apply=scopes.iter().any(|s|s=="--apply");scopes.retain(|s|s!="--apply");assert!(!scopes.is_empty(),"provide package/path scopes");
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();let backup=PathBuf::from(env::var_os("TMPDIR").unwrap_or_else(||"/var/tmp".into())).join("builds/compiler-repair").join(stamp.to_string());
    let mut files:BTreeMap<PathBuf,Vec<Value>>=BTreeMap::new();
    for line in fs::read_to_string(diagnostics).unwrap().lines(){let Ok(value)=serde_json::from_str::<Value>(line)else{continue};let message=&value["message"];if message["level"]!="error"{continue}
        let mut paths=BTreeSet::new();for span in message["spans"].as_array().into_iter().flatten(){if let Some(file)=span["file_name"].as_str(){if scopes.iter().any(|s|file.contains(s)){paths.insert(PathBuf::from(file));}}}
        for path in paths {files.entry(path).or_default().push(message.clone());}
    }
    let mut count=0;
    for (path,messages) in files {let original=fs::read_to_string(&path).unwrap();let mut source=original.clone();let edits=repair(&source,&messages);let mut bound=source.len();let mut changed=0;for (r,replacement) in edits.into_iter().rev(){if r.1>bound{continue}source.replace_range(r.0..r.1,&replacement);bound=r.0;changed+=1;}if changed>0{if apply{let relative=path.strip_prefix(env::current_dir().unwrap()).unwrap_or(&path);let saved=backup.join(relative);fs::create_dir_all(saved.parent().unwrap()).unwrap();fs::write(saved,original).unwrap();fs::write(&path,source).unwrap();}println!("{}: {changed}",path.display());count+=changed}}
    println!("adapted {count} expressions");
    if apply && count>0{println!("backup: {}",backup.display());}
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn exact_typed_span(){let source="fn f(){let n = value();}";let a=source.find("value()").unwrap();let msg=serde_json::json!({"code":{"code":"E0308"},"spans":[{"byte_start":a,"byte_end":a+7,"line_start":1,"column_start":a+1,"text":[{"text":source}],"is_primary":true,"label":"expected `u32`, found `Result<u32, Never>`"}]});let edits=repair(source,&[msg]);assert_eq!(edits.len(),1);assert!(edits.values().next().unwrap().contains("let Ok"));}
    #[test]fn mutable_constructor_extracted_once(){let source="fn f(){let mut pb = Builder::new(); ({ let Ok(value) = pb; value }).draw();}";let edits=repair(source,&[]);assert_eq!(edits.len(),2);assert!(edits.values().any(|s|s=="pb"));}
    #[test]fn stale_text_rejected(){let source="fn f(){let n = value();}";let msg=serde_json::json!({"code":{"code":"E0308"},"spans":[{"byte_start":15,"byte_end":22,"line_start":1,"column_start":16,"text":[{"text":"old source"}],"is_primary":true,"label":"expected `u32`, found `Result<u32, Never>`"}]});assert!(repair(source,&[msg]).is_empty());}
    #[test]fn shifted_prefix_rejected(){let source="\nfn f(){let n = value();}";let a=source.find("value()").unwrap();let msg=serde_json::json!({"code":{"code":"E0308"},"spans":[{"byte_start":a-1,"byte_end":a+6,"line_start":2,"column_start":a,"text":[{"text":source.lines().nth(1).unwrap()}],"is_primary":true,"label":"expected `u32`, found `Result<u32, Never>`"}]});assert!(repair(source,&[msg]).is_empty());}
    fn message(source:&str,code:&str,title:&str,operand:&str,label:&str)->Value{let a=source.find(operand).unwrap();serde_json::json!({"code":{"code":code},"message":title,"rendered":title,"spans":[{"byte_start":a,"byte_end":a+operand.len(),"line_start":1,"column_start":a+1,"text":[{"text":source}],"is_primary":true,"label":label}]})}
    #[test]fn arithmetic_operand_exact_type(){let source="fn f(){ consume() + 2; }";let msg=message(source,"E0369","cannot add to Result<u32, Never>","consume()","Result<u32, Never>");let edits=repair(source,&[msg]);assert_eq!(edits.len(),1);assert!(edits.values().next().unwrap().contains("consume()"));}
    #[test]fn dereference_extracts_constructor_once(){let source="fn f(){let mid = make(); let borrowed = &*mid;}";let msg=message(source,"E0614","type `Result<Box<[u8; 4]>, Never>` cannot be dereferenced","*mid","cannot be dereferenced");let edits=repair(source,&[msg]);let a=source.find("make()").unwrap();assert!(edits.contains_key(&Range(a,a+6)));}
    #[test]fn option_receiver_notes_are_not_evidence(){let source="fn f(){ let value = make(); value.draw(); }";let mut msg=message(source,"E0599","no method named draw found for enum `Option<T>`","draw","method not found");msg["rendered"]=serde_json::json!("note: draw returns Result<(), Never>");assert!(repair(source,&[msg]).is_empty());}
    #[test]fn constructor_from_another_function_is_never_touched(){let source="fn first(){let value=other();} fn second(value: Result<u32,Never>){value.draw();}";let msg=message(source,"E0599","no method draw on `Result<u32, Never>`","draw","method not found");let edits=repair(source,&[msg]);assert!(!edits.values().any(|text|text.contains("other()")));}
}
