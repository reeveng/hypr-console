#![feature(rustc_private)]
extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::{Expr, ExprKind, FnDecl, FnRetTy};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use serde_json::{json, Value};
use std::io::Write;

dylint_linting::impl_late_lint! {
    pub TEXT_ADAPT, Warn, "records typed source spans for adapting text code", TextAdapt::default()
}

#[derive(Default)]
pub struct TextAdapt {
    records: Vec<Value>,
}

fn selected_source(name: &str) -> bool {
    if !name.ends_with(".rs") {
        return false;
    }
    match std::env::var("LEGION_ADAPT_PACKAGES") {
        Ok(packages) => packages
            .split(',')
            .map(str::trim)
            .filter(|package| !package.is_empty())
            .any(|package| name.contains(&format!("{package}/"))),
        Err(_) => {
            name.contains("crates/console-core-video-decoding-")
                || name.contains("crates/console-core-jpeg-2000-files/")
        }
    }
}

fn source(cx: &LateContext<'_>, span: Span) -> Option<Value> {
    if span.from_expansion() || span.is_dummy() {
        return None;
    }
    let file = cx.sess().source_map().lookup_source_file(span.lo());
    let name = match &file.name {
        rustc_span::FileName::Real(real) => real.local_path()?.display().to_string(),
        _ => return None,
    };
    let wanted = selected_source(&name);
    if !wanted {
        return None;
    }
    Some(json!({"file":name,"lo":(span.lo()-file.start_pos).0,"hi":(span.hi()-file.start_pos).0}))
}

fn def_path(cx: &LateContext<'_>, id: rustc_hir::def_id::DefId) -> String {
    format!(
        "{}::{:?}",
        cx.tcx.crate_name(id.krate),
        cx.tcx.def_path(id).data
    )
}
// Canonical identities survive module aliases and similarly named local enums.
fn result_error(cx: &LateContext<'_>, value: ty::Ty<'_>) -> Option<String> {
    let ty::Adt(def, args) = value.kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(rustc_span::sym::Result, def.did()) {
        return None;
    }
    let error = args.type_at(1);
    if error.is_unit() {
        return Some("()".into());
    }
    let ty::Adt(error, parameters) = error.kind() else {
        return None;
    };
    if !parameters.is_empty() {
        return None;
    }
    Some(format!(
        "{}{}",
        cx.tcx.crate_name(error.did().krate),
        cx.tcx.def_path(error.did()).to_string_no_crate_verbose()
    ))
}

fn resolved(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<rustc_hir::def_id::DefId> {
    match expr.kind {
        ExprKind::Path(ref path) => cx.qpath_res(path, expr.hir_id).opt_def_id(),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for TextAdapt {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx rustc_hir::Body<'tcx>,
        span: Span,
        id: rustc_hir::def_id::LocalDefId,
    ) {
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        let Some(location) = source(cx, span) else {
            return;
        };
        let (abi, constant) = match kind {
            rustc_hir::intravisit::FnKind::ItemFn(_, _, header) => (
                header.abi,
                matches!(header.constness, rustc_hir::Constness::Const),
            ),
            rustc_hir::intravisit::FnKind::Method(_, signature) => (
                signature.header.abi,
                matches!(signature.header.constness, rustc_hir::Constness::Const),
            ),
            rustc_hir::intravisit::FnKind::Closure => (rustc_abi::ExternAbi::Rust, false),
        };
        let output = cx.tcx.fn_sig(id).skip_binder().output().skip_binder();
        let result = matches!(output.kind(),ty::TyKind::Adt(def,_) if cx.tcx.is_diagnostic_item(rustc_span::sym::Result,def.did()));
        let trait_impl = matches!(
            cx.tcx.def_kind(cx.tcx.parent(id.to_def_id())),
            rustc_hir::def::DefKind::Impl { of_trait: true }
        );
        let in_trait = matches!(
            cx.tcx.def_kind(cx.tcx.parent(id.to_def_id())),
            rustc_hir::def::DefKind::Trait
        );
        let exported = cx.effective_visibilities.is_exported(id);
        let asynchronous = cx.tcx.asyncness(id.to_def_id()).is_async();
        let entry = matches!(cx.tcx.entry_fn(()), Some((entry, _)) if entry == id.to_def_id());
        let ret = match decl.output {
            FnRetTy::Return(t) => source(cx, t.span),
            FnRetTy::DefaultReturn(s) => source(cx, s),
        };
        self.records.push(json!({"kind":"fn","foreign_abi":abi != rustc_abi::ExternAbi::Rust,"abi":format!("{abi:?}"),"const":constant,"id":def_path(cx,id.to_def_id()),"span":location,"body":source(cx,body.value.span),"ret":ret,"default_ret":matches!(decl.output,FnRetTy::DefaultReturn(_)),"result":result,"result_error":result_error(cx, output),"output":output.to_string(),"trait_impl":trait_impl,"in_trait":in_trait,"exported":exported,"async":asynchronous,"entry":entry,"test":clippy_utils::is_test_function(cx.tcx,id),"params":decl.inputs.iter().map(|t|source(cx,t.span)).collect::<Vec<_>>(),"param_names":body.params.iter().map(|p|source(cx,p.pat.span)).collect::<Vec<_>>(),"param_types":cx.tcx.fn_sig(id).skip_binder().inputs().skip_binder().iter().map(|t|t.to_string()).collect::<Vec<_>>()}));
    }
    fn check_local(&mut self, cx: &LateContext<'tcx>, local: &'tcx rustc_hir::LetStmt<'tcx>) {
        let Some(location) = source(cx, local.span) else {
            return;
        };
        let typed = cx.typeck_results();
        let result_ok = matches!(local.pat.kind, rustc_hir::PatKind::TupleStruct(path, [_], _)
            if cx.qpath_res(&path, local.pat.hir_id).opt_def_id()
                .is_some_and(|id| clippy_utils::is_lang_item_or_ctor(cx, id, rustc_hir::LangItem::ResultOk)));
        let mut bindings = Vec::new();
        local.pat.each_binding(|_,binding,span,name|{
            bindings.push(json!({"name":name.to_string(),"span":source(cx,span),"type":typed.node_type(binding).to_string()}));
        });
        self.records.push(json!({"kind":"local","span":location,"result_ok":result_ok,"bindings":bindings,"init":local.init.and_then(|e|source(cx,e.span)),"pattern":source(cx,local.pat.span),"typed":local.ty.is_some()}));
    }
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some(location) = source(cx, expr.span) else {
            if expr.span.from_expansion() {
                let used = match expr.kind {
                    ExprKind::Path(ref path) => cx.qpath_res(path, expr.hir_id).opt_def_id(),
                    ExprKind::MethodCall(..) => cx.typeck_results().type_dependent_def_id(expr.hir_id),
                    _ => None,
                };
                if let (Some(used), Some(site)) = (used, source(cx, expr.span.source_callsite())) {
                    self.records.push(json!({"kind":"expanded_use","def":def_path(cx, used),"site":site}));
                }
            }
            return;
        };
        let typed = cx.typeck_results();
        let owner = cx.tcx.hir_get_parent_item(expr.hir_id).to_def_id();
        let owner_span = cx.tcx.def_span(owner);
        let mut record = json!({"kind":"expr","span":location,"type":typed.expr_ty(expr).to_string(),"owner":def_path(cx,owner),"owner_kind":format!("{:?}",cx.tcx.def_kind(owner)),"owner_span":source(cx,owner_span),"generated_owner":owner_span.from_expansion() || owner_span.is_dummy()});
        if let Some(error) = result_error(cx, typed.expr_ty(expr)) {
            record["result_error"] = json!(error);
        }
        match expr.kind {
            ExprKind::Call(callee, args) => {
                record["node"] = json!("call");
                record["def"] = json!(resolved(cx, callee).map(|id| def_path(cx, id)));
                record["callee"] = json!(source(cx, callee.span));
                record["args"] = json!(args.iter().map(|e| source(cx, e.span)).collect::<Vec<_>>());
            }
            ExprKind::MethodCall(segment, receiver, args, _) => {
                record["node"] = json!("method");
                record["receiver_mut"] = json!(typed
                    .type_dependent_def_id(expr.hir_id)
                    .is_some_and(|id| matches!(
                        cx.tcx
                            .fn_sig(id)
                            .skip_binder()
                            .inputs()
                            .skip_binder()
                            .first()
                            .map(|t| t.kind()),
                        Some(ty::Ref(_, _, rustc_hir::Mutability::Mut))
                    )));
                record["name"] = json!(segment.ident.to_string());
                record["def"] = json!(typed
                    .type_dependent_def_id(expr.hir_id)
                    .map(|id| def_path(cx, id)));
                record["receiver"] = json!(source(cx, receiver.span));
                record["args"] = json!(args.iter().map(|e| source(cx, e.span)).collect::<Vec<_>>());
            }
            ExprKind::Binary(op, left, right) => {
                record["node"] = json!("binary");
                record["op"] = json!(format!("{:?}", op.node));
                record["left"] = json!(source(cx, left.span));
                record["right"] = json!(source(cx, right.span));
                record["integral"] = json!(typed.expr_ty(left).peel_refs().is_integral());
                record["left_type"] = json!(typed.expr_ty(left).to_string());
                record["right_type"] = json!(typed.expr_ty(right).to_string());
            }
            ExprKind::AssignOp(op, left, right) => {
                record["node"] = json!("assign_op");
                record["op"] = json!(format!("{:?}", op.node));
                record["left"] = json!(source(cx, left.span));
                record["right"] = json!(source(cx, right.span));
                record["integral"] = json!(typed.expr_ty(left).peel_refs().is_integral());
                record["left_type"] = json!(typed.expr_ty(left).to_string());
                record["right_type"] = json!(typed.expr_ty(right).to_string());
            }
            ExprKind::Unary(op, value) => {
                record["node"] = json!("unary");
                record["op"] = json!(format!("{:?}", op));
                record["value"] = json!(source(cx, value.span));
                record["integral"] = json!(typed.expr_ty(value).peel_refs().is_integral());
            }
            ExprKind::Cast(value, _) => {
                record["node"] = json!("cast");
                record["value"] = json!(source(cx, value.span));
                record["from_type"] = json!(typed.expr_ty(value).to_string());
            }
            ExprKind::Ret(value) => {
                record["node"] = json!("return");
                record["value"] = json!(value.and_then(|v| source(cx, v.span)));
            }
            ExprKind::Path(ref path) => {
                record["node"] = json!("path");
                record["def"] = json!(cx
                    .qpath_res(path, expr.hir_id)
                    .opt_def_id()
                    .map(|id| def_path(cx, id)));
            }
            ExprKind::Index(value, index, _) => {
                record["node"] = json!("index");
                record["value"] = json!(source(cx, value.span));
                record["index"] = json!(source(cx, index.span));
                record["value_type"] = json!(typed.expr_ty(value).to_string());
                record["index_type"] = json!(typed.expr_ty(index).to_string());
            }
            ExprKind::Assign(left, right, _) => {
                record["node"] = json!("assign");
                record["left"] = json!(source(cx, left.span));
                record["right"] = json!(source(cx, right.span));
            }
            _ => {
                record["node"] = json!("other");
            }
        }
        self.records.push(record);
    }
    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        if self.records.is_empty() {
            return;
        }
        let files: Vec<_> = cx.sess().source_map().files().iter().cloned().collect();
        for file in files {
            let name = match &file.name {
                rustc_span::FileName::Real(real) => match real.local_path() {
                    Some(path) => path.display().to_string(),
                    None => continue,
                },
                _ => continue,
            };
            if !selected_source(&name) {
                continue;
            }
            let Some(text) = file.src.as_ref() else {
                continue;
            };
            let mut hash = 0xcbf29ce484222325u64;
            for byte in text.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            self.records.push(json!({"kind":"source","span":{"file":name,"lo":0,"hi":text.len()},"fnv1a64":format!("{hash:016x}"),"bytes":text.len()}));
        }
        let target = match cx.sess().opts.test {
            true => "-tests",
            false => "",
        };
        let (directory, prefix) = match std::env::var_os("LEGION_ADAPT_OUTPUT") {
            Some(directory) => (std::path::PathBuf::from(directory), "legion-facts"),
            None => (
                std::env::var_os("TMPDIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from("/var/tmp"))
                    .join("builds"),
                "legion-media-facts",
            ),
        };
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "{prefix}-{}{target}.json",
            cx.tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE)
        ));
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "{}", serde_json::to_string(&self.records).unwrap()).unwrap();
    }
}
