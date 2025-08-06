extern crate alloc;

use heaped::arena::{DroplessArena, TypedArena};
use alloc::{string::String, vec::Vec, format};

#[derive(Debug)]
struct AstNode {
  kind: String,
  children: Vec<usize>,
  span: (usize, usize),
}

impl Drop for AstNode {
  fn drop(&mut self) {
  }
}

#[derive(Debug)]
struct HirNode {
  kind: String, 
  type_info: String,
}

impl Drop for HirNode {
  fn drop(&mut self) {
  }
}

struct StringInterner {
  interned_strings: DroplessArena,
  string_map: Vec<(&'static str, usize)>,
}

impl StringInterner {
  fn new() -> Self {
    Self {
      interned_strings: DroplessArena::new(),
      string_map: Vec::new(),
    }
  }
  
  fn intern(&mut self, s: &str) -> &'static str {
    for &(existing, _) in &self.string_map {
      if existing == s {
        return existing;
      }
    }
    
    let interned = self.interned_strings.alloc_str(s).expect("should intern string");
    let static_str = unsafe { core::mem::transmute::<&str, &'static str>(interned) };
    self.string_map.push((static_str, self.string_map.len()));
    static_str
  }
}

struct Compiler {
  interner: StringInterner,
  hir_arena: TypedArena<HirNode>,
}

impl Compiler {
  fn new() -> Self {
    Self {
      interner: StringInterner::new(),
      hir_arena: TypedArena::new(),
    }
  }
  
  fn parse_stage(&mut self, _source: &str) -> Vec<String> {
    let ast_arena = TypedArena::<AstNode>::new();
    let token_arena = DroplessArena::new();
    
    let _tokens = token_arena.alloc_slice(&["fn", "main", "(", ")", "{", "return", "42", ";", "}"]).expect("should allocate tokens");
    
    let main_fn = self.interner.intern("main");
    let _return_kw = self.interner.intern("return");
    
    let literal_node = ast_arena.alloc(AstNode {
      kind: "Literal".to_string(),
      children: Vec::new(),
      span: (6, 8),
    }).expect("should allocate AST node");
    
    let return_stmt = ast_arena.alloc(AstNode {
      kind: "Return".to_string(), 
      children: vec![0],
      span: (5, 8),
    }).expect("should allocate AST node");
    
    let fn_body = ast_arena.alloc(AstNode {
      kind: "Block".to_string(),
      children: vec![1],
      span: (4, 9),
    }).expect("should allocate AST node");
    
    let fn_decl = ast_arena.alloc(AstNode {
      kind: "Function".to_string(),
      children: vec![2],
      span: (0, 9),
    }).expect("should allocate AST node");
    
    let mut ast_summaries = Vec::new();
    ast_summaries.push(format!("Function {} with {} children", main_fn, fn_decl.children.len()));
    ast_summaries.push(format!("Block with {} statements", fn_body.children.len()));
    ast_summaries.push(format!("Return statement at span {:?}", return_stmt.span));
    ast_summaries.push(format!("Literal at span {:?}", literal_node.span));
    
    ast_summaries
  }
  
  fn lower_to_hir(&mut self, ast_summaries: Vec<String>) -> Vec<String> {
    let mut hir_summaries = Vec::new();
    
    for summary in ast_summaries {
      if summary.contains("Function") {
        let hir_fn = self.hir_arena.alloc(HirNode {
          kind: "HirFunction".to_string(),
          type_info: "fn() -> i32".to_string(),
        }).expect("should allocate HIR node");
        hir_summaries.push(format!("HIR: {} with type {}", hir_fn.kind, hir_fn.type_info));
      } else if summary.contains("Return") {
        let hir_ret = self.hir_arena.alloc(HirNode {
          kind: "HirReturn".to_string(),
          type_info: "i32".to_string(),
        }).expect("should allocate HIR node");
        hir_summaries.push(format!("HIR: {} returning {}", hir_ret.kind, hir_ret.type_info));
      }
    }
    
    hir_summaries
  }
  
  fn stress_test_allocations(&mut self) {
    for _ in 0..10 {
      let temp_arena = DroplessArena::new();
      
      let large_buffer = temp_arena.alloc_slice(&vec![0u8; 4096]).expect("should allocate large buffer");
      assert_eq!(large_buffer.len(), 4096);
      
      for i in 0..100 {
        let temp_string = temp_arena.alloc_str(&format!("temp_string_{}", i)).expect("should allocate temp string");
        assert!(temp_string.contains(&i.to_string()));
      }
    }
    
    for i in 0..50 {
      let persistent_node = self.hir_arena.alloc(HirNode {
        kind: format!("PersistentNode{}", i),
        type_info: "persistent".to_string(),
      }).expect("should allocate persistent node");
      assert!(persistent_node.kind.contains(&i.to_string()));
    }
  }
}

fn main() {
  let mut compiler = Compiler::new();
  
  let ast_results = compiler.parse_stage("fn main() { return 42; }");
  assert_eq!(ast_results.len(), 4);
  assert!(ast_results[0].contains("Function main"));
  
  let hir_results = compiler.lower_to_hir(ast_results);
  assert_eq!(hir_results.len(), 2);
  assert!(hir_results[0].contains("HirFunction"));
  assert!(hir_results[1].contains("HirReturn"));
  
  compiler.stress_test_allocations();
  
  println!("Compiler simulation completed successfully");
}