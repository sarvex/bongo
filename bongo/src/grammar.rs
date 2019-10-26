// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod builder;
pub mod examples;
pub mod nullables;
pub mod transform;

use crate::pdisplay::LayoutDisplay;
use crate::utils::{breadth_first_search, Name};
use codefmt::Layout;
use std::{
  cmp,
  collections::{BTreeMap, BTreeSet},
  ops,
};

fn ref_eq<T>(a: &T, b: &T) -> bool {
  (a as *const T) == (b as *const T)
}

fn ref_cmp<T>(a: &T, b: &T) -> cmp::Ordering {
  (a as *const T).cmp(&(b as *const T))
}

/// A trait which carries the underlying types for a grammar.
///
/// This allows us to specify a family of types at once as a type parameter
/// instead of forcing us to provide a number of type variables with a long list
/// of bounds.
///
/// This type is not instantiated, and will typically be a zero-sized type. It's
/// constrained by the standard set of derivable operations in order to make
/// derivations of types that use it simple.
pub trait ElementTypes:
  Copy + Clone + Eq + PartialEq + PartialOrd + Ord + std::fmt::Debug + 'static
{
  /// The type used to identify each possible terminal.
  ///
  /// Terminals must be cloneable, and must be Ord to be used as a key in a map.
  type Term: Clone
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + LayoutDisplay
    + std::fmt::Debug
    + 'static;

  // The type used to identify each possible non-terminal.
  type NonTerm: Clone
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + LayoutDisplay
    + std::fmt::Debug
    + 'static;

  // The type used to identify each production.
  type ActionKey: Clone
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + std::fmt::Debug
    + 'static;

  type ActionValue: Clone + std::fmt::Debug + 'static;
}

/// A terminal element.
///
/// This is a simple terminal type compatible with `ElementTypes`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Terminal(Name);

impl Terminal {
  pub fn new(s: &str) -> Self {
    Terminal(Name::new(s))
  }
}

impl LayoutDisplay for Terminal {
  fn disp(&self) -> codefmt::Layout {
    let name = self.0.str();
    Layout::text(name)
  }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NonTerminal(Name);

impl NonTerminal {
  pub fn new(s: &str) -> Self {
    NonTerminal(Name::new(s))
  }
}

impl LayoutDisplay for NonTerminal {
  fn disp(&self) -> codefmt::Layout {
    Layout::juxtapose(&[
      Layout::text("<"),
      Layout::text(self.0.str()),
      Layout::text(">"),
    ])
  }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct BaseElementTypes;

impl ElementTypes for BaseElementTypes {
  type Term = Terminal;
  type NonTerm = NonTerminal;
  type ActionKey = Name;
  type ActionValue = ();
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Element<E: ElementTypes> {
  Term(E::Term),
  NonTerm(E::NonTerm),
}

impl<E: ElementTypes> Element<E> {
  pub fn as_term(&self) -> Option<&E::Term> {
    match self {
      Element::NonTerm(_) => None,
      Element::Term(t) => Some(t),
    }
  }
  /// Gets an element as a nonterm. Panics if it is not a nonterm.
  pub fn as_nonterm(&self) -> Option<&E::NonTerm> {
    match self {
      Element::NonTerm(nt) => Some(nt),
      Element::Term(_) => None,
    }
  }

  pub fn clone_as_other<E2>(&self) -> Element<E2>
  where
    E2: ElementTypes<Term = E::Term, NonTerm = E::NonTerm>,
  {
    match self {
      Element::Term(t) => Element::Term(t.clone()),
      Element::NonTerm(nt) => Element::NonTerm(nt.clone()),
    }
  }
}

impl<E: ElementTypes> LayoutDisplay for Element<E> {
  fn disp(&self) -> codefmt::Layout {
    match self {
      Element::Term(t) => t.disp(),
      Element::NonTerm(nt) => nt.disp(),
    }
  }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ProductionElement<E: ElementTypes> {
  identifier: Option<Name>,
  element: Element<E>,
}

impl<E: ElementTypes> ProductionElement<E> {
  pub fn new_with_name(name: Name, e: Element<E>) -> Self {
    ProductionElement {
      identifier: Some(name),
      element: e,
    }
  }

  pub fn new(name: Option<Name>, e: Element<E>) -> Self {
    ProductionElement {
      identifier: name,
      element: e,
    }
  }

  pub fn new_empty(e: Element<E>) -> Self {
    ProductionElement {
      identifier: None,
      element: e,
    }
  }

  pub fn id(&self) -> Option<&Name> {
    self.identifier.as_ref()
  }

  pub fn elem(&self) -> &Element<E> {
    &self.element
  }

  pub fn clone_as_other<E2>(&self) -> ProductionElement<E2>
  where
    E2: ElementTypes<Term = E::Term, NonTerm = E::NonTerm>,
  {
    ProductionElement {
      identifier: self.identifier.clone(),
      element: self.element.clone_as_other(),
    }
  }
}

impl<E: ElementTypes> LayoutDisplay for ProductionElement<E> {
  fn disp(&self) -> codefmt::Layout {
    match &self.identifier {
      Some(name) => Layout::juxtapose(&[
        name.layout(),
        Layout::text(": "),
        self.element.disp(),
      ]),
      None => self.element.disp(),
    }
  }
}

impl<E: ElementTypes> From<Element<E>> for ProductionElement<E> {
  fn from(e: Element<E>) -> ProductionElement<E> {
    ProductionElement {
      identifier: None,
      element: e,
    }
  }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Production<E: ElementTypes> {
  action_key: E::ActionKey,
  elements: Vec<ProductionElement<E>>,
}

impl<E: ElementTypes> Production<E> {
  pub fn new(
    action_key: E::ActionKey,
    elements: Vec<ProductionElement<E>>,
  ) -> Production<E> {
    Production {
      action_key,
      elements,
    }
  }

  pub fn prod_elements(&self) -> &Vec<ProductionElement<E>> {
    &self.elements
  }

  pub fn elements_iter(&self) -> impl Iterator<Item = &Element<E>> {
    self.elements.iter().map(|prod_elem| &prod_elem.element)
  }

  pub fn element_at(&self, index: usize) -> Option<&Element<E>> {
    self.elements.get(index).map(|prod_elem| &prod_elem.element)
  }

  pub fn action_key(&self) -> &E::ActionKey {
    &self.action_key
  }
}

impl<E: ElementTypes> LayoutDisplay for Production<E> {
  fn disp(&self) -> Layout {
    let elements =
      Layout::wrap(self.elements.iter().map(|x| x.disp()).collect::<Vec<_>>());
    Layout::juxtapose(&[
      elements,
      Layout::text(" => "),
      Layout::text(format!("{:?}", self.action_key)),
    ])
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProdKey<E: ElementTypes> {
  head: E::NonTerm,
  action_key: E::ActionKey,
}

#[derive(Clone, Debug)]
pub struct ProdAndHead<'a, E: ElementTypes> {
  head: &'a E::NonTerm,
  prod: &'a Production<E>,
}

impl<'a, E: ElementTypes> ProdAndHead<'a, E> {
  fn head(&self) -> &'a E::NonTerm {
    self.head
  }
  fn prod(&self) -> &'a Production<E> {
    self.prod
  }
  fn key(&self) -> ProdKey<E> {
    ProdKey {
      head: self.head.clone(),
      action_key: self.prod.action_key().clone(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Rule<E: ElementTypes> {
  head: E::NonTerm,
  prods: Vec<Production<E>>,
}

impl<E: ElementTypes> Rule<E> {
  pub fn new(head: E::NonTerm, prods: Vec<Production<E>>) -> Self {
    Rule { head, prods }
  }

  pub fn head(&self) -> &E::NonTerm {
    &self.head
  }

  pub fn prods(&self) -> &Vec<Production<E>> {
    &self.prods
  }
}

impl<E: ElementTypes> LayoutDisplay for Rule<E> {
  fn disp(&self) -> Layout {
    let prod_layouts: Vec<_> =
      self.prods.iter().map(|prod| prod.disp()).collect();
    Layout::stack(prod_layouts)
  }
}

/// A grammar
#[derive(Clone)]
pub struct Grammar<E: ElementTypes> {
  start_symbol: E::NonTerm,
  rule_set: BTreeMap<E::NonTerm, Rule<E>>,
  action_map: BTreeMap<E::ActionKey, E::ActionValue>,
}

impl<E: ElementTypes> std::fmt::Debug for Grammar<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let mut dbg_struct = f.debug_struct("Grammar");
    dbg_struct.field("Terms", &self.get_terminals().collect::<Vec<_>>());
    unimplemented!()
  }
}

impl<E: ElementTypes> Grammar<E> {
  pub fn new(
    start: E::NonTerm,
    rule_set: impl IntoIterator<Item = Rule<E>>,
    action_map: BTreeMap<E::ActionKey, E::ActionValue>,
  ) -> Result<Self, GrammarErrors<E>> {
    let g = Grammar {
      start_symbol: start,
      rule_set: rule_set
        .into_iter()
        .map(|r| (r.head().clone(), r))
        .collect(),
      action_map,
    };

    g.check_grammar().map(|_| g)
  }

  pub fn start_nt(&self) -> &E::NonTerm {
    &self.start_symbol
  }

  pub fn rule_set(&self) -> &BTreeMap<E::NonTerm, Rule<E>> {
    &self.rule_set
  }

  pub fn get_rule(&self, nt: &E::NonTerm) -> Option<&Rule<E>> {
    self.rule_set.get(nt)
  }

  pub fn get_action_map(&self) -> BTreeMap<ProdKey<E>, ProdAndHead<E>> {
    use std::collections::btree_map::Entry;
    let mut map: BTreeMap<ProdKey<E>, ProdAndHead<E>> = BTreeMap::new();
    for (nt_head, rule) in &self.rule_set {
      for prod in &rule.prods {
        match map.entry(ProdKey {
          head: nt_head.clone(),
          action_key: prod.action_key().clone(),
        }) {
          Entry::Occupied(_) => {
            panic!("Can be only one example of each parameter. Duplicated param: {:?}", prod.action_key());
          }
          Entry::Vacant(vac) => {
            vac.insert(ProdAndHead {
              head: nt_head,
              prod: prod,
            });
          }
        }
      }
    }

    map
  }

  pub fn prod_and_heads(&self) -> impl Iterator<Item = ProdAndHead<E>> {
    self.rule_set.iter().flat_map(|(head, rule)| {
      rule
        .prods()
        .iter()
        .map(move |prod| ProdAndHead { head, prod })
    })
  }

  fn get_elements(&self) -> impl Iterator<Item = &Element<E>> {
    self
      .rule_set
      .values()
      .flat_map(|r| &r.prods)
      .flat_map(|p| p.elements_iter())
  }

  fn get_terminals(&self) -> impl Iterator<Item = &E::Term> {
    self.get_elements().filter_map(|e| e.as_term())
  }

  fn get_nonterminals(&self) -> impl Iterator<Item = &E::NonTerm> {
    self.get_elements().filter_map(|e| e.as_nonterm())
  }

  fn nonterminals_without_rules(&self) -> BTreeSet<&E::NonTerm> {
    self
      .get_nonterminals()
      .filter(move |nt| !self.rule_set.contains_key(nt))
      .collect()
  }

  fn rules_without_prods(&self) -> BTreeSet<&E::NonTerm> {
    self
      .rule_set()
      .values()
      .filter(|r| r.prods.is_empty())
      .map(|r| &r.head)
      .collect()
  }

  fn reachable_nonterms(&self) -> BTreeSet<&E::NonTerm> {
    breadth_first_search(std::iter::once(&self.start_symbol), |nt| {
      match self.get_rule(nt) {
        Some(rule) => rule
          .prods
          .iter()
          .flat_map(|p| p.elements_iter())
          .filter_map(|e| e.as_nonterm())
          .collect(),
        None => BTreeSet::new(),
      }
    })
  }

  fn unreachable_nonterms(&self) -> BTreeSet<&E::NonTerm> {
    let reachable_nonterms = self.reachable_nonterms();
    self
      .get_nonterminals()
      .filter(|nt| !reachable_nonterms.contains(nt))
      .collect()
  }
}

#[derive(Clone, Debug)]
pub struct GrammarErrors<E: ElementTypes> {
  unreachable_nonterms: BTreeSet<E::NonTerm>,
  nonterms_without_rules: BTreeSet<E::NonTerm>,
  rules_without_prods: BTreeSet<E::NonTerm>,
}

impl<E: ElementTypes> GrammarErrors<E> {
  fn into_result(self) -> Result<(), Self> {
    if self.unreachable_nonterms.is_empty()
      && self.nonterms_without_rules.is_empty()
      && self.rules_without_prods.is_empty()
    {
      Ok(())
    } else {
      Err(self)
    }
  }
}

impl<E: ElementTypes> Grammar<E> {
  fn check_grammar(&self) -> Result<(), GrammarErrors<E>> {
    GrammarErrors {
      unreachable_nonterms: self
        .unreachable_nonterms()
        .into_iter()
        .cloned()
        .collect(),
      nonterms_without_rules: self
        .nonterminals_without_rules()
        .into_iter()
        .cloned()
        .collect(),
      rules_without_prods: self
        .rules_without_prods()
        .into_iter()
        .cloned()
        .collect(),
    }
    .into_result()
  }
}

impl<E: ElementTypes> LayoutDisplay for Grammar<E> {
  fn disp(&self) -> Layout {
    let mut stack = Vec::new();
    for (k, v) in &self.rule_set {
      let name_layout = if &self.start_symbol == k {
        Layout::juxtapose(&[Layout::text("*"), k.disp()])
      } else {
        k.disp()
      };

      stack.push(Layout::juxtapose(&[name_layout, Layout::text(":")]));
      stack.push(Layout::juxtapose(&[Layout::text("  "), v.disp()]));
    }
    Layout::stack(stack)
  }
}

// ------------

/// A simple deref wrapper that ensures that two references _must_ be the same during
/// comparison. This ensures that we can't accidentally incorporate refs from different parents together.
struct ParentRef<'a, T>(&'a T);

impl<'a, T> ParentRef<'a, T> {
  fn new(r: &'a T) -> Self {
    ParentRef(r)
  }
}

impl<T> cmp::PartialEq for ParentRef<'_, T> {
  fn eq(&self, other: &Self) -> bool {
    assert!(ref_eq(self.0, other.0));
    true
  }
}

impl<T> cmp::Eq for ParentRef<'_, T> {}

impl<T> cmp::PartialOrd for ParentRef<'_, T> {
  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl<T> cmp::Ord for ParentRef<'_, T> {
  fn cmp(&self, other: &Self) -> cmp::Ordering {
    assert!(ref_eq(self.0, other.0));
    cmp::Ordering::Equal
  }
}

impl<'a, T> ops::Deref for ParentRef<'a, T> {
  type Target = &'a T;
  fn deref(&self) -> &&'a T {
    &self.0
  }
}

impl<T> Clone for ParentRef<'_, T> {
  fn clone(&self) -> Self {
    ParentRef(self.0)
  }
}

impl<T> Copy for ParentRef<'_, T> {}

// ------------

#[derive(Clone, Copy)]
struct NoCompare<T>(T);

impl<T> cmp::PartialEq for NoCompare<T> {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl<T> cmp::Eq for NoCompare<T> {}

impl<T> cmp::PartialOrd for NoCompare<T> {
  fn partial_cmp(&self, _: &Self) -> Option<cmp::Ordering> {
    Some(cmp::Ordering::Equal)
  }
}

impl<T> cmp::Ord for NoCompare<T> {
  fn cmp(&self, _: &Self) -> cmp::Ordering {
    cmp::Ordering::Equal
  }
}

impl<T> ops::Deref for NoCompare<T> {
  type Target = T;
  fn deref(&self) -> &T {
    &self.0
  }
}

// ------------

struct RefCompare<'a, T>(&'a T);

impl<T> cmp::PartialEq for RefCompare<'_, T> {
  fn eq(&self, other: &Self) -> bool {
    ref_eq(self.0, other.0)
  }
}

impl<T> cmp::Eq for RefCompare<'_, T> {}

impl<T> cmp::PartialOrd for RefCompare<'_, T> {
  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl<T> cmp::Ord for RefCompare<'_, T> {
  fn cmp(&self, other: &Self) -> cmp::Ordering {
    ref_cmp(self.0, other.0)
  }
}

impl<'a, T> ops::Deref for RefCompare<'a, T> {
  type Target = &'a T;
  fn deref(&self) -> &&'a T {
    &self.0
  }
}

impl<T> Clone for RefCompare<'_, T> {
  fn clone(&self) -> Self {
    RefCompare(self.0)
  }
}

impl<T> Copy for RefCompare<'_, T> {}

// ------------

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleRef<'a, E: ElementTypes> {
  grammar: ParentRef<'a, Grammar<E>>,
  head: &'a E::NonTerm,
  prods: &'a Vec<Production<E>>,
}

impl<'a, E: ElementTypes> RuleRef<'a, E> {
  pub fn head(&self) -> &E::NonTerm {
    self.head
  }

  pub fn prods(&self) -> Vec<ProdRef<'a, E>> {
    self
      .prods
      .iter()
      .map(|prod| ProdRef {
        grammar: self.grammar,
        head: self.head,
        prod: RefCompare(prod),
        action_value: NoCompare(
          self.grammar.action_map.get(&prod.action_key).unwrap(),
        ),
      })
      .collect()
  }
}

// ------------

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProdRef<'a, E: ElementTypes> {
  grammar: ParentRef<'a, Grammar<E>>,
  head: &'a E::NonTerm,
  prod: RefCompare<'a, Production<E>>,
  action_value: NoCompare<&'a E::ActionValue>,
}

impl<'a, E: ElementTypes> ProdRef<'a, E> {
  pub fn prod_elements(&self) -> &'a Vec<ProductionElement<E>> {
    &self.prod.elements
  }

  pub fn elements_iter(&self) -> impl Iterator<Item = &'a Element<E>> {
    self.prod.elements_iter()
  }

  pub fn element_at(&self, index: usize) -> Option<&'a Element<E>> {
    self.prod.element_at(index)
  }

  pub fn action_key(&self) -> &'a E::ActionKey {
    self.prod.action_key()
  }

  pub fn action_value(&self) -> &'a E::ActionValue {
    *self.action_value
  }
}
