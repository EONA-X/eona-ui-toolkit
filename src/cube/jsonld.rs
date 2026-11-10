//! A JSON-LD subset reader for the chart profile.
//!
//! Supported: `@context` (the built-in URL, inline objects, arrays, `null`), `@id`,
//! `@type`, `@value` with `@type` or `@language`, `@graph`, `@reverse` and reverse
//! terms, CURIEs, `@vocab`, `@language`, term-level `@type` coercion (`@id` or a
//! datatype). Remote contexts are never fetched, and `@list` is rejected.
//!
//! The document is expanded into a flat set of nodes, then read against the profile
//! contract (see `docs/` of the vocabulary repository, `eonax-chart-profile`).

use super::*;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const BUILTIN_CONTEXT: &str = include_str!("context.jsonld");

const QB: &str = "http://purl.org/linked-data/cube#";
const CHART: &str = "https://eona-x.eu/shape/chart-profile#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

fn qb(s: &str) -> String {
    format!("{QB}{s}")
}
fn chart(s: &str) -> String {
    format!("{CHART}{s}")
}
fn xsd(s: &str) -> String {
    format!("{XSD}{s}")
}

type Res<T> = Result<T, CubeError>;

// ---------------------------------------------------------------- contexts

#[derive(Clone)]
struct TermDef {
    /// For a reverse term, the property it reverses.
    target: String,
    reverse: bool,
    ty: Option<String>,
}

#[derive(Clone, Default)]
struct Ctx {
    vocab: Option<String>,
    language: Option<String>,
    terms: HashMap<String, TermDef>,
}

impl Ctx {
    fn apply(&mut self, v: &Value, path: &str) -> Res<()> {
        match v {
            Value::Null => *self = Ctx::default(),
            Value::String(s) if s == CONTEXT_URL => {
                let doc: Value = serde_json::from_str(BUILTIN_CONTEXT).expect("built-in context");
                self.apply(&doc["@context"], path)?;
            }
            Value::String(_) => {
                return Err(CubeError::new(path, "remote contexts are not fetched"));
            }
            Value::Array(a) => {
                for c in a {
                    self.apply(c, path)?;
                }
            }
            Value::Object(m) => self.apply_object(m, path)?,
            _ => return Err(CubeError::new(path, "invalid @context")),
        }
        Ok(())
    }

    fn apply_object(&mut self, m: &Map<String, Value>, path: &str) -> Res<()> {
        for (k, v) in m {
            match k.as_str() {
                "@vocab" => self.vocab = v.as_str().map(str::to_owned),
                "@language" => self.language = v.as_str().map(str::to_owned),
                "@base" | "@version" | "@protected" | "@import" => {}
                _ => match v {
                    Value::Null => {
                        self.terms.remove(k);
                    }
                    Value::String(s) => {
                        self.terms.insert(
                            k.clone(),
                            TermDef {
                                target: s.clone(),
                                reverse: false,
                                ty: None,
                            },
                        );
                    }
                    Value::Object(d) => {
                        let (target, reverse) = match (d.get("@reverse"), d.get("@id")) {
                            (Some(Value::String(r)), _) => (r.clone(), true),
                            (None, Some(Value::String(i))) => (i.clone(), false),
                            (None, None) => (k.clone(), false),
                            _ => {
                                return Err(CubeError::new(
                                    format!("{path}/{k}"),
                                    "invalid term definition",
                                ))
                            }
                        };
                        let ty = d.get("@type").and_then(Value::as_str).map(str::to_owned);
                        self.terms.insert(k.clone(), TermDef { target, reverse, ty });
                    }
                    _ => {
                        return Err(CubeError::new(
                            format!("{path}/{k}"),
                            "invalid term definition",
                        ))
                    }
                },
            }
        }
        Ok(())
    }

    /// Expands a term, CURIE or IRI. `vocab` allows terms and `@vocab`.
    fn resolve(&self, s: &str, vocab: bool, depth: u8) -> Option<String> {
        if depth > 16 {
            return None;
        }
        if vocab {
            if let Some(d) = self.terms.get(s) {
                if d.target == s {
                    return self.vocab.as_ref().map(|v| format!("{v}{s}"));
                }
                return self.resolve(&d.target, true, depth + 1);
            }
        }
        if let Some((prefix, rest)) = s.split_once(':') {
            if prefix == "_" || rest.starts_with("//") {
                return Some(s.to_owned());
            }
            if let Some(d) = self.terms.get(prefix) {
                return Some(format!("{}{rest}", self.resolve(&d.target, true, depth + 1)?));
            }
            return Some(s.to_owned());
        }
        if vocab {
            self.vocab.as_ref().map(|v| format!("{v}{s}"))
        } else {
            Some(s.to_owned())
        }
    }
}

// ------------------------------------------------------------------- graph

#[derive(Clone, Debug, PartialEq)]
struct Lit {
    value: String,
    datatype: String,
    lang: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum Val {
    Lit(Lit),
    Ref(String),
}

#[derive(Default)]
struct Node {
    types: Vec<String>,
    props: Vec<(String, Val)>,
}

#[derive(Default)]
struct Graph {
    /// Insertion order, which is document order.
    ids: Vec<String>,
    nodes: HashMap<String, Node>,
    blank: usize,
}

impl Graph {
    fn node(&mut self, id: &str) -> &mut Node {
        if !self.nodes.contains_key(id) {
            self.ids.push(id.to_owned());
        }
        self.nodes.entry(id.to_owned()).or_default()
    }

    /// An RDF graph is a set of triples: a value repeated verbatim is one triple.
    fn add(&mut self, id: &str, prop: &str, val: Val) {
        let n = self.node(id);
        if !n.props.iter().any(|(p, v)| p == prop && *v == val) {
            n.props.push((prop.to_owned(), val));
        }
    }

    fn fresh(&mut self) -> String {
        self.blank += 1;
        format!("_:b{}", self.blank)
    }

    fn expand_node(&mut self, parent: &Ctx, obj: &Map<String, Value>, path: &str) -> Res<String> {
        let mut ctx = parent.clone();
        if let Some(c) = obj.get("@context") {
            ctx.apply(c, &format!("{path}/@context"))?;
        }
        let id = match obj.get("@id") {
            Some(Value::String(s)) => ctx
                .resolve(s, false, 0)
                .ok_or_else(|| CubeError::new(path, "invalid @id"))?,
            Some(_) => return Err(CubeError::new(path, "@id must be a string")),
            None => self.fresh(),
        };
        self.node(&id);
        for (k, v) in obj {
            let kp = format!("{path}/{k}");
            match k.as_str() {
                "@context" | "@id" | "@index" => {}
                "@type" => {
                    let items = match v {
                        Value::String(_) => std::slice::from_ref(v),
                        Value::Array(a) => a.as_slice(),
                        _ => return Err(CubeError::new(kp, "@type must be a string or array")),
                    };
                    for t in items {
                        let s = t
                            .as_str()
                            .ok_or_else(|| CubeError::new(&kp, "@type must be strings"))?;
                        let iri = ctx.resolve(s, true, 0).unwrap_or_else(|| s.to_owned());
                        let n = self.node(&id);
                        if !n.types.contains(&iri) {
                            n.types.push(iri);
                        }
                    }
                }
                "@graph" => {
                    for (i, item) in as_items(v).iter().enumerate() {
                        let o = item
                            .as_object()
                            .ok_or_else(|| CubeError::new(format!("{kp}/{i}"), "expected a node object"))?;
                        self.expand_node(&ctx, o, &format!("{kp}/{i}"))?;
                    }
                }
                "@reverse" => {
                    let m = v
                        .as_object()
                        .ok_or_else(|| CubeError::new(&kp, "@reverse must be an object"))?;
                    for (rk, rv) in m {
                        let Some(iri) = ctx.resolve(rk, true, 0) else { continue };
                        for (i, item) in as_items(rv).iter().enumerate() {
                            let ip = format!("{kp}/{rk}/{i}");
                            let child = self.child_id(&ctx, item, &ip)?;
                            self.add(&child, &iri, Val::Ref(id.clone()));
                        }
                    }
                }
                _ if k.starts_with('@') => {}
                _ => {
                    let Some(iri) = ctx.resolve(k, true, 0) else { continue };
                    let def = ctx.terms.get(k).cloned();
                    let reverse = def.as_ref().is_some_and(|d| d.reverse);
                    for (i, item) in as_items(v).iter().enumerate() {
                        let ip = format!("{kp}/{i}");
                        if reverse {
                            let child = self.child_id(&ctx, item, &ip)?;
                            self.add(&child, &iri, Val::Ref(id.clone()));
                        } else if let Some(t) = self.expand_value(&ctx, def.as_ref(), item, &ip)? {
                            self.add(&id, &iri, t);
                        }
                    }
                }
            }
        }
        Ok(id)
    }

    /// The id of a node given as an object, or referenced by a string.
    fn child_id(&mut self, ctx: &Ctx, item: &Value, path: &str) -> Res<String> {
        match item {
            Value::Object(o) => self.expand_node(ctx, o, path),
            Value::String(s) => ctx
                .resolve(s, false, 0)
                .ok_or_else(|| CubeError::new(path, "invalid reference")),
            _ => Err(CubeError::new(path, "expected a node")),
        }
    }

    fn expand_value(
        &mut self,
        ctx: &Ctx,
        def: Option<&TermDef>,
        item: &Value,
        path: &str,
    ) -> Res<Option<Val>> {
        let ty = def.and_then(|d| d.ty.as_deref());
        Ok(Some(match item {
            Value::Null => return Ok(None),
            Value::String(s) => match ty {
                Some("@id") => Val::Ref(
                    ctx.resolve(s, false, 0)
                        .ok_or_else(|| CubeError::new(path, "invalid reference"))?,
                ),
                Some(t) => Val::Lit(Lit {
                    value: s.clone(),
                    datatype: ctx.resolve(t, true, 0).unwrap_or_else(|| t.to_owned()),
                    lang: None,
                }),
                None => match &ctx.language {
                    Some(l) => Val::Lit(Lit {
                        value: s.clone(),
                        datatype: LANG_STRING.to_owned(),
                        lang: Some(l.clone()),
                    }),
                    None => Val::Lit(Lit {
                        value: s.clone(),
                        datatype: xsd("string"),
                        lang: None,
                    }),
                },
            },
            Value::Number(_) | Value::Bool(_) => native(item),
            Value::Array(_) => return Err(CubeError::new(path, "nested arrays are not supported")),
            Value::Object(o) => {
                if o.contains_key("@list") {
                    return Err(CubeError::new(path, "@list is not supported"));
                }
                if let Some(v) = o.get("@value") {
                    let mut lit = match native(v) {
                        Val::Lit(l) => l,
                        Val::Ref(_) => unreachable!(),
                    };
                    if let Value::String(s) = v {
                        lit.value = s.clone();
                    } else if !matches!(v, Value::Number(_) | Value::Bool(_)) {
                        return Err(CubeError::new(path, "@value must be a string, number or boolean"));
                    }
                    if let Some(l) = o.get("@language").and_then(Value::as_str) {
                        lit.datatype = LANG_STRING.to_owned();
                        lit.lang = Some(l.to_owned());
                    } else if let Some(t) = o.get("@type").and_then(Value::as_str) {
                        lit.datatype = ctx.resolve(t, true, 0).unwrap_or_else(|| t.to_owned());
                    }
                    Val::Lit(lit)
                } else {
                    Val::Ref(self.expand_node(ctx, o, path)?)
                }
            }
        }))
    }
}

fn as_items(v: &Value) -> &[Value] {
    match v {
        Value::Array(a) => a.as_slice(),
        _ => std::slice::from_ref(v),
    }
}

/// A JSON number or boolean as a literal with its natural datatype.
fn native(v: &Value) -> Val {
    let (value, dt) = match v {
        Value::Number(n) if n.is_i64() || n.is_u64() => (n.to_string(), "integer"),
        Value::Number(n) => (n.to_string(), "double"),
        Value::Bool(b) => (b.to_string(), "boolean"),
        Value::String(s) => (s.clone(), "string"),
        _ => (String::new(), "string"),
    };
    Val::Lit(Lit {
        value,
        datatype: xsd(dt),
        lang: None,
    })
}

// ----------------------------------------------------------------- profile

/// Reads a chart-profile document. See the module docs for the supported subset.
pub fn from_jsonld(text: &str) -> Result<Cube, CubeError> {
    let doc: Value = serde_json::from_str(text)
        .map_err(|e| CubeError::new("", format!("invalid JSON: {e}")))?;
    let mut g = Graph::default();
    let root = Ctx::default();
    match &doc {
        Value::Object(o) => {
            g.expand_node(&root, o, "")?;
        }
        Value::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                let o = item
                    .as_object()
                    .ok_or_else(|| CubeError::new(format!("/{i}"), "expected a node object"))?;
                g.expand_node(&root, o, &format!("/{i}"))?;
            }
        }
        _ => return Err(CubeError::new("", "expected a JSON-LD object or array")),
    }
    read_cube(&g)
}

fn props<'a>(n: &'a Node, iri: &str) -> Vec<&'a Val> {
    n.props.iter().filter(|(p, _)| p == iri).map(|(_, t)| t).collect()
}

fn literal<'a>(t: &'a Val, path: &str, prop: &str) -> Res<&'a Lit> {
    match t {
        Val::Lit(l) => Ok(l),
        Val::Ref(_) => Err(CubeError::new(path, format!("{prop} must be a literal"))),
    }
}

fn lang_map(terms: Vec<&Val>, path: &str, prop: &str) -> Res<LangMap> {
    let mut out = Vec::new();
    for t in terms {
        let l = literal(t, path, prop)?;
        if l.datatype == xsd("string") {
            out.push((String::new(), l.value.clone()));
        } else if l.datatype == LANG_STRING {
            out.push((l.lang.clone().unwrap_or_default(), l.value.clone()));
        } else {
            return Err(CubeError::new(
                path,
                format!("{prop} must be xsd:string or rdf:langString"),
            ));
        }
    }
    Ok(LangMap(out))
}

fn display_id(id: &str) -> String {
    id.to_owned()
}

fn read_cube(g: &Graph) -> Res<Cube> {
    let datasets: Vec<&String> = g
        .ids
        .iter()
        .filter(|id| g.nodes[*id].types.contains(&qb("DataSet")))
        .collect();
    if datasets.len() != 1 {
        return Err(CubeError::new(
            "",
            format!("expected exactly one qb:DataSet, found {}", datasets.len()),
        ));
    }
    let ds_id = datasets[0];
    let ds = &g.nodes[ds_id];
    let dpath = display_id(ds_id);

    let title = lang_map(props(ds, &format!("{RDFS}label")), &dpath, "rdfs:label")?;
    if title.is_empty() {
        return Err(CubeError::new(&dpath, "the dataset needs at least one rdfs:label"));
    }
    let comments = lang_map(props(ds, &format!("{RDFS}comment")), &dpath, "rdfs:comment")?;
    let unit = match props(ds, &chart("unit")).as_slice() {
        [] => None,
        [t] => {
            let l = literal(t, &dpath, "chart:unit")?;
            if l.datatype != xsd("string") {
                return Err(CubeError::new(&dpath, "chart:unit must be xsd:string"));
            }
            Some(l.value.clone())
        }
        _ => return Err(CubeError::new(&dpath, "chart:unit may appear at most once")),
    };

    // Observations: every qb:Observation, and nothing else may carry qb:dataSet. As in
    // the shapes, an observation has exactly one qb:dataSet and it is the dataset.
    let mut raw: Vec<Raw> = Vec::new();
    for id in &g.ids {
        let n = &g.nodes[id];
        let sets = props(n, &qb("dataSet"));
        let typed = n.types.contains(&qb("Observation"));
        let path = display_id(id);
        if !typed {
            if !sets.is_empty() {
                return Err(CubeError::new(
                    path,
                    "a node with qb:dataSet must be typed qb:Observation",
                ));
            }
            continue;
        }
        if !matches!(sets.as_slice(), [Val::Ref(r)] if r == ds_id) {
            return Err(CubeError::new(
                path,
                "a qb:Observation has exactly one qb:dataSet, the dataset",
            ));
        }
        raw.push(read_obs(n, &path)?);
        if raw.len() > MAX_OBSERVATIONS {
            return Err(CubeError::new(
                &dpath,
                format!("too many observations (at most {MAX_OBSERVATIONS})"),
            ));
        }
    }

    // One axis, one date datatype.
    let mut axis: Option<Axis> = None;
    for r in &raw {
        let a = match r.key {
            AxisKey::Category(_) => Axis::Category,
            AxisKey::Date(_) => Axis::Date(r.date_kind.expect("date kind")),
        };
        match axis {
            None => axis = Some(a),
            Some(Axis::Category) if a != Axis::Category => {
                return Err(CubeError::new(&r.path, "all observations must use the same axis property"))
            }
            Some(Axis::Date(_)) if a == Axis::Category => {
                return Err(CubeError::new(&r.path, "all observations must use the same axis property"))
            }
            Some(x) if x != a => {
                return Err(CubeError::new(&r.path, "all dates must have the same datatype"))
            }
            _ => {}
        }
    }

    // Series: all or none, at most MAX_SERIES.
    let with_series = raw.iter().filter(|r| r.series.is_some()).count();
    if with_series != 0 && with_series != raw.len() {
        return Err(CubeError::new(
            &dpath,
            "either every observation has chart:series or none does",
        ));
    }
    let names: BTreeSet<&String> = raw.iter().filter_map(|r| r.series.as_ref()).collect();
    if names.len() > MAX_SERIES {
        return Err(CubeError::new(
            &dpath,
            format!("at most {MAX_SERIES} distinct series are allowed, found {}", names.len()),
        ));
    }
    let series: Vec<String> = names.into_iter().cloned().collect();

    // (axis key, series) is unique.
    let mut seen = BTreeSet::new();
    for r in &raw {
        if !seen.insert((r.key.clone(), r.series.clone())) {
            return Err(CubeError::new(
                &r.path,
                format!("duplicate observation for {:?}", r.key.as_str()),
            ));
        }
    }

    // A category sorts by the smallest chart:order any of its observations carries.
    let mut cat_order: BTreeMap<&AxisKey, i64> = BTreeMap::new();
    for r in &raw {
        if let Some(o) = r.order {
            let e = cat_order.entry(&r.key).or_insert(o);
            *e = (*e).min(o);
        }
    }
    let day_of = |k: &AxisKey| match k {
        AxisKey::Date(s) => parse_day(s),
        AxisKey::Category(_) => None,
    };
    let mut observations: Vec<Obs> = raw
        .iter()
        .map(|r| Obs {
            key: r.key.clone(),
            series: r
                .series
                .as_ref()
                .map(|s| series.binary_search(s).expect("series listed")),
            value: r.value,
            order: r.order,
        })
        .collect();
    let is_cat = matches!(axis, Some(Axis::Category) | None);
    observations.sort_by(|a, b| {
        let rank = |o: &Obs| {
            if is_cat {
                cat_order.get(&o.key).copied().unwrap_or(i64::MAX)
            } else {
                0
            }
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| day_of(&a.key).cmp(&day_of(&b.key)))
            .then_with(|| a.key.cmp(&b.key))
            .then_with(|| a.series.cmp(&b.series))
    });

    Ok(Cube {
        id: if ds_id.starts_with("_:") { String::new() } else { ds_id.clone() },
        title,
        description: if comments.is_empty() { None } else { Some(comments) },
        axis: axis.unwrap_or(Axis::Category),
        unit,
        series,
        observations,
    })
}

struct Raw {
    path: String,
    key: AxisKey,
    date_kind: Option<DateKind>,
    series: Option<String>,
    value: f64,
    order: Option<i64>,
}

fn read_obs(n: &Node, path: &str) -> Res<Raw> {
    let cats = props(n, &chart("category"));
    let dates = props(n, &chart("date"));
    let (key, date_kind) = match (cats.as_slice(), dates.as_slice()) {
        ([c], []) => {
            let l = literal(c, path, "chart:category")?;
            if l.datatype != xsd("string") {
                return Err(CubeError::new(path, "chart:category must be xsd:string"));
            }
            (AxisKey::Category(l.value.clone()), None)
        }
        ([], [d]) => {
            let l = literal(d, path, "chart:date")?;
            let kind = [
                ("date", DateKind::Date),
                ("gYearMonth", DateKind::YearMonth),
                ("gYear", DateKind::Year),
            ]
            .into_iter()
            .find(|(t, _)| l.datatype == xsd(t))
            .map(|(_, k)| k)
            .ok_or_else(|| {
                CubeError::new(path, "chart:date must be xsd:date, xsd:gYearMonth or xsd:gYear")
            })?;
            if !valid_date(kind, &l.value) {
                let name = match kind {
                    DateKind::Date => "xsd:date",
                    DateKind::YearMonth => "xsd:gYearMonth",
                    DateKind::Year => "xsd:gYear",
                };
                return Err(CubeError::new(path, format!("invalid {name}: {:?}", l.value)));
            }
            (AxisKey::Date(l.value.clone()), Some(kind))
        }
        _ => {
            return Err(CubeError::new(
                path,
                "an observation needs exactly one of chart:category or chart:date",
            ))
        }
    };

    let value = match props(n, &chart("value")).as_slice() {
        [t] => {
            let l = literal(t, path, "chart:value")?;
            if !["decimal", "double", "float", "integer"]
                .iter()
                .any(|d| l.datatype == xsd(d))
            {
                return Err(CubeError::new(
                    path,
                    "chart:value must be xsd:decimal, xsd:double, xsd:float or xsd:integer",
                ));
            }
            let local = l.datatype.strip_prefix(XSD).unwrap_or_default();
            let lex = l.value.trim();
            if matches!(lex, "NaN" | "INF" | "+INF" | "-INF") && matches!(local, "double" | "float") {
                return Err(CubeError::new(
                    path,
                    format!("chart:value must be a finite number, got {:?}", l.value),
                ));
            }
            if !valid_number(local, lex) {
                return Err(CubeError::new(
                    path,
                    format!("chart:value {:?} is not a valid xsd:{local}", l.value),
                ));
            }
            match lex.parse::<f64>() {
                Ok(v) if v.is_finite() => v,
                _ => {
                    return Err(CubeError::new(
                        path,
                        format!("chart:value must be a finite number, got {:?}", l.value),
                    ))
                }
            }
        }
        _ => return Err(CubeError::new(path, "an observation needs exactly one chart:value")),
    };

    let series = match props(n, &chart("series")).as_slice() {
        [] => None,
        [t] => {
            let l = literal(t, path, "chart:series")?;
            if l.datatype != xsd("string") {
                return Err(CubeError::new(path, "chart:series must be xsd:string"));
            }
            Some(l.value.clone())
        }
        _ => return Err(CubeError::new(path, "chart:series may appear at most once")),
    };

    let order = match props(n, &chart("order")).as_slice() {
        [] => None,
        [t] => {
            let l = literal(t, path, "chart:order")?;
            if l.datatype != xsd("integer") {
                return Err(CubeError::new(path, "chart:order must be xsd:integer"));
            }
            Some(
                l.value
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| CubeError::new(path, "chart:order must be xsd:integer"))?,
            )
        }
        _ => return Err(CubeError::new(path, "chart:order may appear at most once")),
    };

    Ok(Raw {
        path: path.to_owned(),
        key,
        date_kind,
        series,
        value,
        order,
    })
}

/// The XSD lexical space of the numeric datatypes the profile allows (after
/// whitespace collapse); the special double and float values are handled by the caller.
fn valid_number(datatype: &str, s: &str) -> bool {
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let decimal = |m: &str| match m.split_once('.') {
        None => digits(m),
        Some((i, f)) => (i.is_empty() || digits(i)) && (f.is_empty() || digits(f)) && !(i.is_empty() && f.is_empty()),
    };
    match datatype {
        "integer" => digits(s),
        "decimal" => decimal(s),
        "double" | "float" => match s.split_once(['e', 'E']) {
            None => decimal(s),
            Some((m, e)) => decimal(m) && digits(e.strip_prefix(['+', '-']).unwrap_or(e)),
        },
        _ => false,
    }
}

/// `(year, month, day)` of an XSD date, year-month or year with an optional
/// timezone and a possibly negative year; month and day default to 1.
fn parse_day(s: &str) -> Option<(i64, u32, u32)> {
    let digits = |p: &str, len: usize| p.len() == len && p.bytes().all(|b| b.is_ascii_digit());
    // Timezone: `Z` or `(+|-)hh:mm`, at most 14:00.
    let body = if let Some(b) = s.strip_suffix('Z') {
        b
    } else if s.len() > 6 && matches!(s.as_bytes()[s.len() - 6], b'+' | b'-') && s.as_bytes()[s.len() - 3] == b':' {
        let (hh, mm) = (&s[s.len() - 5..s.len() - 3], &s[s.len() - 2..]);
        if !(digits(hh, 2) && digits(mm, 2)) {
            return None;
        }
        let (h, m): (u32, u32) = (hh.parse().ok()?, mm.parse().ok()?);
        if m > 59 || h > 14 || (h == 14 && m != 0) {
            return None;
        }
        &s[..s.len() - 6]
    } else {
        s
    };
    let (neg, rest) = match body.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, body),
    };
    let mut parts = rest.split('-');
    let y = parts.next()?;
    if y.len() < 4 || !y.bytes().all(|b| b.is_ascii_digit()) || (y.len() > 4 && y.starts_with('0')) {
        return None;
    }
    let year: i64 = y.parse().ok()?;
    let year = if neg { -year } else { year };
    let month = match parts.next() {
        None => 1,
        Some(m) if digits(m, 2) && (1..=12).contains(&m.parse::<u32>().ok()?) => m.parse().ok()?,
        Some(_) => return None,
    };
    let day = match parts.next() {
        None => 1,
        Some(d) if digits(d, 2) => d.parse().ok()?,
        Some(_) => return None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((year, month, day))
}

fn valid_date(kind: DateKind, s: &str) -> bool {
    let Some((y, m, d)) = parse_day(s) else { return false };
    let segments = |body: &str| body.trim_start_matches('-').split('-').count();
    let core = s.strip_suffix('Z').unwrap_or(s);
    let core = if core.len() > 6 && matches!(core.as_bytes()[core.len() - 6], b'+' | b'-') && core.as_bytes()[core.len() - 3] == b':' {
        &core[..core.len() - 6]
    } else {
        core
    };
    match (kind, segments(core)) {
        (DateKind::Year, 1) | (DateKind::YearMonth, 2) => true,
        (DateKind::Date, 3) => {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let days = match m {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if leap => 29,
                _ => 28,
            };
            (1..=days).contains(&d)
        }
        _ => false,
    }
}
