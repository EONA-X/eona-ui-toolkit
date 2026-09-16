//! The intermediate representation every stage reads or writes.
//!
//! This is the contract between stages: `parse` fills it from `syn`, `classify`
//! refines `PropKind`, `matrix` derives render cells from it, `abi` serialises it,
//! and the emitters read it. Nothing downstream re-parses Rust.

use serde::{Deserialize, Serialize};

/// Which tier a component belongs to. Only `Presentational` components are
/// generated; `Interactive` is recorded so the ABI can prove it was excluded on
/// purpose rather than missed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Presentational,
    Interactive,
}

/// What the generator decided to do with a component. Every one of the crate's
/// components carries exactly one of these in the ABI, so a component can never
/// silently vanish from the output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Status {
    /// Generated normally.
    Ok,
    /// Needs an `abi/overrides.toml` entry before it can be generated.
    NeedsOverride { reason: String },
    /// The verify gate rejected it; a hand-written port is required.
    Quarantined { reason: String },
    /// Deliberately out of scope. `reason` is the rule that excluded it.
    Excluded { reason: String },
}

/// Where something came from, for diagnostics that a human can act on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub line: usize,
}

/// The classified shape of a prop. `classify` maps every Rust type onto exactly
/// one of these or fails loudly — there is no `Unknown` that silently becomes
/// `any` in TypeScript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PropKind {
    /// `AttrValue`, `String`, `&'static str`, `Cow<'static, str>`.
    Text,
    Bool,
    /// Any integer or float; `rust` keeps the original for the TS comment.
    Num { rust: String },
    /// A fieldless enum — becomes a TypeScript string union.
    UnitEnum { name: String },
    /// `Vec<T>`.
    List { item: Box<PropKind> },
    /// A tuple type, which two props in the crate use as an inline pair:
    /// `Vec<(&'static str, &'static str)>` at `src/molecules/nav_dropdown.rs:18`
    /// and `src/molecules/pillar_card.rs:9`. Becomes a TypeScript tuple; its
    /// elements are matrix axes exactly as a struct's fields are.
    Tuple { items: Vec<PropKind> },
    /// A plain struct reachable from a prop; its fields are matrix axes too.
    Struct { name: String },
    /// `Html` / `Children` — a slot. Becomes React `children` or a render prop.
    Slot,
    /// `Callback<T>` — cannot be pre-rendered; forces `NeedsOverride`.
    Callback { arg: String },
}

/// How a prop's default is expressed in the Yew source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "default", rename_all = "kebab-case")]
pub enum PropDefault {
    /// No `#[prop_or*]` attribute — the prop is required.
    Required,
    /// `#[prop_or_default]` — `Default::default()`.
    DefaultTrait,
    /// `#[prop_or(expr)]`; `expr` is the token text, `ts` the literal when the
    /// expression is a constant the generator could evaluate.
    Expr { expr: String, ts: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub name: String,
    /// The Rust type verbatim, for diagnostics and the ABI.
    pub rust_ty: String,
    pub kind: PropKind,
    /// `Option<T>` in the Rust source.
    pub optional: bool,
    pub default: PropDefault,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    /// `None` for a zero-prop component.
    pub props_ty: Option<String>,
    pub tier: Tier,
    pub status: Status,
    pub props: Vec<Prop>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropEnum {
    pub name: String,
    pub variants: Vec<String>,
    /// The variant carrying `#[default]`, if the enum derives `Default`.
    ///
    /// Without it `#[prop_or_default]` on an enum prop has no TypeScript
    /// default and the generated component reads `undefined` where Yew reads
    /// `BadgeVariant::Default` (`src/atoms/onto_badge.rs:24`) — the one place
    /// the React API would otherwise differ from the Yew one. `serde(default)`
    /// so an `ir.json` written before this field still deserialises.
    #[serde(default)]
    pub default_variant: Option<String>,
    pub span: Span,
}

/// A plain struct reachable from a prop. Its fields are matrix axes, because a
/// component may branch on them (`props_table.rs:59` branches on
/// `PropSpec.default`; `annotation.rs:33-41` matches three ways on
/// `LiteralValue.language`/`.datatype`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlainStruct {
    pub name: String,
    pub fields: Vec<Prop>,
    pub span: Span,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolkit {
    pub components: Vec<Component>,
    pub enums: Vec<PropEnum>,
    pub structs: Vec<PlainStruct>,
    /// sha256 over the sorted contents of `src/**/*.rs`, so the ABI can prove it
    /// matches the source it was generated from.
    pub src_hash: String,
}

impl Toolkit {
    pub fn component(&self, name: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.name == name)
    }
    pub fn unit_enum(&self, name: &str) -> Option<&PropEnum> {
        self.enums.iter().find(|e| e.name == name)
    }
    pub fn plain_struct(&self, name: &str) -> Option<&PlainStruct> {
        self.structs.iter().find(|s| s.name == name)
    }
    /// The components the generator will actually emit.
    pub fn generated(&self) -> impl Iterator<Item = &Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.status, Status::Ok))
    }
}
