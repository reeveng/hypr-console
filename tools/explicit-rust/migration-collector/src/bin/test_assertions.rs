use std::{collections::BTreeMap, env, fs, path::{Path,PathBuf}};
use proc_macro2::{Group,TokenStream,TokenTree,Span};
use syn::{visit::{self,Visit}, spanned::Spanned, parse::Parser, Expr};
#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]struct Range(usize,usize);
struct Walker<'a>{source:&'a str,lines:Vec<usize>,edits:BTreeMap<Range,String>,tests:u32,closures:u32,changed:bool}
fn clean(tokens:TokenStream)->TokenStream{let mut out=TokenStream::new();let mut it=tokens.into_iter().peekable();while let Some(t)=it.next(){match t{
TokenTree::Punct(ref p)if p.as_char()=='$'&&matches!(it.peek(),Some(TokenTree::Ident(_)))=>{},
TokenTree::Punct(ref p)if p.as_char()=='$'&&matches!(it.peek(),Some(TokenTree::Group(_)))=>{if let Some(TokenTree::Group(g))=it.next(){out.extend(clean(g.stream()));}if matches!(it.peek(),Some(TokenTree::Punct(p))if matches!(p.as_char(),'*'|'+'|'?')){it.next();}else{let mut look=it.clone();if matches!(look.next(),Some(TokenTree::Punct(_)))&&matches!(look.next(),Some(TokenTree::Punct(p))if matches!(p.as_char(),'*'|'+'|'?')){it.next();it.next();}}},
TokenTree::Group(g)=>{let mut n=Group::new(g.delimiter(),clean(g.stream()));n.set_span(g.span());out.extend([TokenTree::Group(n)]);},t=>out.extend([t])}}out}
impl Walker<'_>{
 fn range(&self,span:Span)->Range{let off=|lc:proc_macro2::LineColumn|{let start=self.lines[lc.line-1];let line=self.source[start..].split('\n').next().unwrap();start+line.char_indices().map(|(n,_)|n).chain(std::iter::once(line.len())).nth(lc.column).unwrap_or(lc.column)};let mut start=off(span.start());if start>0&&self.source.as_bytes()[start-1]==b'$'{start-=1}Range(start,off(span.end()))}
 fn text(&self,span:Span)->&str{let r=self.range(span);&self.source[r.0..r.1]}
 fn templates(&mut self,tokens:TokenStream){for t in tokens{if let TokenTree::Group(g)=t{let c=clean(g.stream());if let Ok(file)=syn::parse2::<syn::File>(c.clone()){self.visit_file(&file)}else if let Ok(block)=syn::parse2::<syn::Block>(TokenTree::Group({let mut n=Group::new(proc_macro2::Delimiter::Brace,c);n.set_span(g.span());n}).into()){self.visit_block(&block)}else{self.templates(g.stream())}}}}
}
impl<'ast> Visit<'ast> for Walker<'_>{
 fn visit_item_fn(&mut self,f:&'ast syn::ItemFn){
  let test=f.attrs.iter().any(|a|a.path().is_ident("test"));
  if test && matches!(f.sig.output,syn::ReturnType::Default){
   if f.attrs.iter().any(|a|a.path().is_ident("should_panic")){eprintln!("manual: should_panic {}",f.sig.ident);return}
   let r=self.range(f.block.span());self.edits.insert(Range(r.0,r.0),"-> core::result::Result<(), crate::test_support::Failure> ".into());self.edits.insert(Range(r.1-1,r.1-1),"\nOk(())\n".into());self.tests+=1;self.changed=true;visit::visit_item_fn(self,f);self.tests-=1;
  }else if test && self.text(f.sig.output.span()).contains("test_support::Failure") {self.tests+=1;visit::visit_item_fn(self,f);self.tests-=1;}else{visit::visit_item_fn(self,f)}
 }
 fn visit_expr_closure(&mut self,e:&'ast syn::ExprClosure){self.closures+=1;visit::visit_expr_closure(self,e);self.closures-=1;}
 fn visit_expr_method_call(&mut self,e:&'ast syn::ExprMethodCall){
  if self.tests>0 && self.closures==0 && matches!(e.method.to_string().as_str(),"unwrap"|"expect"|"unwrap_err"|"expect_err"){
   let method=match e.method.to_string().as_str(){"unwrap"|"expect"=>"value","unwrap_err"|"expect_err"=>"error",_=>unreachable!()};
   let receiver=self.text(e.receiver.span());let new=format!("crate::test_support::Extract::{method}({receiver}, line!())?");self.edits.insert(self.range(e.span()),new);
  }
  visit::visit_expr_method_call(self,e);
 }
 fn visit_macro(&mut self,m:&'ast syn::Macro){
  let name=m.path.segments.last().unwrap().ident.to_string();
  if self.tests>0 && self.closures==0 && matches!(name.as_str(),"assert"|"assert_eq"|"assert_ne"|"panic"|"unreachable"){
   let parser=syn::punctuated::Punctuated::<Expr,syn::Token![,]>::parse_terminated;
   let Ok(args)=parser.parse2(clean(m.tokens.clone()))else{return};
   for arg in &args { self.visit_expr(arg); }
   let failure="return Err(crate::test_support::Failure::Assertion { line: line!() })";
   let new=match name.as_str(){
    "assert" if !args.is_empty()=>format!("match {{ {} }} {{ true => (), false => {{ {failure} }} }}",self.text(args[0].span())),
    "assert_eq"|"assert_ne" if args.len()>=2=>format!("{{ let left = &({}); let right = &({}); match left {} right {{ true => (), false => {{ {failure} }} }} }}",self.text(args[0].span()),self.text(args[1].span()),if name=="assert_eq"{"=="}else{"!="}),
    "panic"|"unreachable"=>format!("{{ {failure} }}"),
    _=>return,
   };
   self.edits.insert(self.range(m.span()),new);return
  }
  let parser=syn::punctuated::Punctuated::<Expr,syn::Token![,]>::parse_terminated;
  if let Ok(args)=parser.parse2(clean(m.tokens.clone())){for arg in args{self.visit_expr(&arg)}}else{self.templates(m.tokens.clone())}
 }
}
const SUPPORT:&str=r#"#[derive(Debug)]
pub(crate) enum Failure {
    Assertion { line: u32 },
    MissingValue { line: u32 },
    UnexpectedError { line: u32 },
    UnexpectedValue { line: u32 },
}

pub(crate) trait Extract {
    type Value;
    type Error;

    fn value(self, line: u32) -> Result<Self::Value, Failure>;

    fn error(self, line: u32) -> Result<Self::Error, Failure>;
}

impl<T> Extract for Option<T> {
    type Value = T;
    type Error = ();

    fn value(self, line: u32) -> Result<T, Failure> {
        match self {
            Some(value) => Ok(value),
            None => Err(Failure::MissingValue { line }),
        }
    }

    fn error(self, line: u32) -> Result<(), Failure> {
        match self {
            Some(_) => Err(Failure::UnexpectedValue { line }),
            None => Ok(()),
        }
    }
}

impl<T, E> Extract for Result<T, E> {
    type Value = T;
    type Error = E;

    fn value(self, line: u32) -> Result<T, Failure> {
        match self {
            Ok(value) => Ok(value),
            Err(_) => Err(Failure::UnexpectedError { line }),
        }
    }

    fn error(self, line: u32) -> Result<E, Failure> {
        match self {
            Ok(_) => Err(Failure::UnexpectedValue { line }),
            Err(error) => Ok(error),
        }
    }
}
"#;
fn migrate(source:&str)->(String,bool){
 let tree=syn::parse_file(source).unwrap();let mut lines=vec![0];lines.extend(source.bytes().enumerate().filter_map(|(i,b)|(b==b'\n').then_some(i+1)));let mut w=Walker{source,lines,edits:BTreeMap::new(),tests:0,closures:0,changed:false};w.visit_file(&tree);
 // Render nested edits from the inside out, carrying transformed children into parents.
 let mut pending=w.edits.into_iter().collect::<Vec<_>>();pending.sort_by_key(|(r,_)|(r.1-r.0,r.0));let mut applied:Vec<(Range,String)>=vec![];
 for (r,mut replacement) in pending{
  let old=&source[r.0..r.1];let children=applied.iter().filter(|(child,_)|r.0<=child.0&&r.1>=child.1&&*child!=r).cloned().collect::<Vec<_>>();
  for (child,value) in &children{let child_old=&source[child.0..child.1];if replacement.contains(child_old){replacement=replacement.replacen(child_old,value,1)}}
  if !children.is_empty(){applied.retain(|(child,_)|!(r.0<=child.0&&r.1>=child.1&&*child!=r));}
  if replacement!=old{applied.push((r,replacement));}
 }
 applied.sort_by_key(|(r,_)|*r);let mut output=source.to_owned();for (r,new) in applied.into_iter().rev(){output.replace_range(r.0..r.1,&new)}(output,w.changed)
}
fn files(path:&Path,out:&mut Vec<PathBuf>){if path.is_dir(){for e in fs::read_dir(path).unwrap(){files(&e.unwrap().path(),out)}}else if path.extension().is_some_and(|s|s=="rs"){out.push(path.to_owned())}}
fn main(){let args=env::args().skip(1).collect::<Vec<_>>();let apply=args.iter().any(|s|s=="--apply");let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();let backup=PathBuf::from(env::var_os("TMPDIR").unwrap_or_else(||"/var/tmp".into())).join("builds/test-assertion-repair").join(stamp.to_string());let save=|path:&Path|{if path.exists(){let relative=path.strip_prefix(env::current_dir().unwrap()).unwrap_or(path);let saved=backup.join(relative);fs::create_dir_all(saved.parent().unwrap()).unwrap();fs::copy(path,saved).unwrap();}};
for root in args.into_iter().filter(|s|s!="--apply"){let root=PathBuf::from(root);let src=root.join("src");let mut sources=vec![];files(&src,&mut sources);let mut needed=false;for path in sources{if path.ends_with("test_support.rs"){continue}let source=fs::read_to_string(&path).unwrap();let (output,changed)=migrate(&source);if output!=source{if apply{save(&path);fs::write(&path,output).unwrap();}println!("{}",path.display());}needed|=changed;}if needed{let support=src.join("test_support.rs");if apply && !support.exists(){fs::write(&support,SUPPORT).unwrap();}let lib=src.join("lib.rs");let mut s=fs::read_to_string(&lib).unwrap();if !s.contains("mod test_support;"){s.push_str("\n#[cfg(test)]\nmod test_support;\n");if apply{save(&lib);fs::write(lib,s).unwrap();}}}}
if apply{println!("backup: {}",backup.display());}}
#[cfg(test)]mod tests{use super::*;
 #[test]fn assertion_order(){let (s,changed)=migrate("#[test] fn test(){assert_eq!(left(), right());}");assert!(changed);assert!(s.contains("let left = &(left()); let right = &(right());"));assert!(syn::parse_file(&s).is_ok());}
 #[test]fn nested_unwrap(){let (s,_)=migrate("#[test] fn test(){assert_eq!(f().unwrap(), 2);}");assert!(s.contains("Extract::value"));assert!(!s.contains("unwrap"));}
 #[test]fn ordinary_code_preserved(){let s="fn f(){ assert!(true); }";assert_eq!(migrate(s).0,s);}
 #[test]fn macro_template(){let (s,changed)=migrate("macro_rules! test {($name:ident)=>{#[test] fn $name(){assert!(true);}}}");assert!(changed);assert!(!s.contains("assert!"));assert!(s.contains("fn $name()"));}
 #[test]fn macro_repetition(){let (s,changed)=migrate("macro_rules! test {($name:ident,$($token:expr),*)=>{#[test] fn $name(){ $( assert_eq!(f().unwrap(),$token); )* }}}");assert!(changed);assert!(!s.contains("assert_eq!"));assert!(!s.contains("unwrap"));assert!(s.contains("$token"));assert!(s.contains("$("));}
}
