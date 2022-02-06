mod arg;
pub mod to_js;

use super::utils::is_space;
use super::{js, Parser, ParserError, SourceLocation, TagType};

// parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
// TODO support upper case tag names
pub fn parse_tag(p: &mut Parser, v_else_allowed: bool) -> Result<Tag, ParserError> {
    let mut tag = Tag {
        type_: TagType::Open,
        name: SourceLocation(p.current_char, 0),
        args: VueTagArgs::new(),
        is_custom_component: false,
    };

    let mut is_close_tag = false;
    let mut c = p
        .seek_one()
        .ok_or(ParserError::eof("parse_tag check closure tag"))?;

    if c == '/' {
        tag.type_ = TagType::Close;
        tag.name.0 += 1;
        p.current_char += 1;
        is_close_tag = true;
    } else if c == '!' {
        p.current_char += 1;

        let mut doctype = "DOCTYPE ".chars();
        while let Some(doctype_c) = doctype.next() {
            let c = p.must_read_one()?;
            if doctype_c != c {
                return Err(ParserError::new(
                    "parse_tag",
                    format!(
                        "expected '{}' of \"<!DOCTYPE\" but got '{}'",
                        doctype_c.to_string(),
                        c.to_string()
                    ),
                ));
            }
        }

        while p.must_read_one()? != '>' {}
        return Ok(Tag {
            type_: TagType::DocType,
            name: SourceLocation::empty(),
            args: VueTagArgs::new(),
            is_custom_component: false,
        });
    }

    // Parse names
    loop {
        match p.must_read_one()? {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => {}
            _ => {
                p.current_char -= 1;
                tag.name.1 = p.current_char;
                break;
            }
        };
    }

    if tag.name.1 == 0 {
        return Err(ParserError::new("parse_tag", "expected tag name"));
    }

    tag.is_custom_component = is_tag_name_a_custom_component(p, &tag.name);

    // Parse args
    loop {
        c = p.must_read_one_skip_spacing()?;
        c = match arg::try_parse(p, c, &mut tag.args, v_else_allowed, tag.is_custom_component)? {
            Some(next_char) => next_char,
            None => c,
        };

        match c {
            '/' => {
                if is_close_tag {
                    return Err(ParserError::new(
                        "parse_tag",
                        "/ not allowed after name in closeing tag",
                    ));
                }
                c = p.must_read_one_skip_spacing()?;
                if c != '>' {
                    return Err(ParserError::new(
                        "parse_tag",
                        format!("expected > but got '{}'", c.to_string()),
                    ));
                }
                tag.type_ = TagType::OpenAndClose;
                return Ok(tag);
            }
            '>' => return Ok(tag),
            c if is_space(c) => {}
            c => {
                return Err(ParserError::new(
                    "parse_tag",
                    format!("unexpected character '{}'", c.to_string()),
                ))
            }
        }
    }
}

fn add_or_set<T>(list: &mut Option<Vec<T>>, add: T) {
    if let Some(list) = list.as_mut() {
        list.push(add);
    } else {
        *list = Some(vec![add]);
    }
}

pub fn compile(p: &mut Parser) -> Result<Vec<Child>, ParserError> {
    let mut compile_result = Child::parse_children(p, &mut Vec::new())?;
    loop {
        if compile_result.1.eq(p, "template".chars()) {
            return Ok(compile_result.0);
        } else {
            compile_result = Child::parse_children(p, &mut Vec::new())?;
        }
    }
}

#[derive(Debug, Clone)]
pub enum Child {
    Tag(Tag, Vec<Child>),
    Text(SourceLocation),
    Var(String),
}

impl Child {
    fn parse_children(
        p: &mut Parser,
        parents_tag_names: &mut Vec<SourceLocation>,
    ) -> Result<(Vec<Self>, SourceLocation), ParserError> {
        let mut resp: Vec<Child> = Vec::with_capacity(1);
        let mut inside_v_if = false;
        loop {
            let (text_node, compile_now) = Self::compile_text_node(p)?;
            if let Some(node) = text_node {
                inside_v_if = false;
                resp.push(node);
            }

            match compile_now {
                CompileAfterTextNode::Tag => {
                    let tag = parse_tag(p, inside_v_if)?;

                    if let Some(modifier) = tag.args.modifier.as_ref() {
                        inside_v_if = match modifier {
                            arg::VueTagModifier::If(_) => true,
                            arg::VueTagModifier::ElseIf(_) => true,
                            _ => false,
                        };
                    } else {
                        inside_v_if = false;
                    }

                    match tag.type_ {
                        TagType::Close => {
                            let mut found = false;
                            for parent in parents_tag_names.iter().rev() {
                                if tag.name.eq_self(p, parent) {
                                    found = true;
                                    break;
                                }
                            }
                            if found || tag.name.eq(p, "template".chars()) {
                                return Ok((resp, tag.name));
                            }
                        }
                        TagType::Open => {
                            parents_tag_names.push(tag.name.clone());

                            let local_variables = tag.args.new_local_variables.as_ref();

                            // Add the new local variables if there where some
                            if let Some(new_local_variables) = local_variables {
                                for var_name in new_local_variables {
                                    if let Some(count) = p.local_variables.get_mut(var_name) {
                                        *count += 1;
                                    } else {
                                        p.local_variables.insert(var_name.clone(), 1);
                                    }
                                }
                            }

                            let compile_children_result =
                                Self::parse_children(p, parents_tag_names);

                            let tag_name = parents_tag_names.pop().unwrap();
                            let (children, closing_tag_name) = compile_children_result?;

                            // Remove the local variables we above inserted
                            if let Some(new_local_variables) = local_variables {
                                for var_name in new_local_variables {
                                    if let Some(count) = p.local_variables.get_mut(var_name) {
                                        if *count == 1 {
                                            p.local_variables.remove(var_name);
                                        } else {
                                            *count -= 1;
                                        }
                                    }
                                }
                            }

                            resp.push(Self::Tag(tag, children));

                            let correct_closing_tag = tag_name.eq_self(p, &closing_tag_name);
                            if !correct_closing_tag {
                                return Ok((resp, closing_tag_name));
                            }
                        }
                        TagType::OpenAndClose => {
                            resp.push(Self::Tag(tag, Vec::new()));
                        }
                        TagType::DocType => {} // Skip this tag
                    };
                }
                CompileAfterTextNode::Var => {
                    inside_v_if = false;
                    resp.push(Self::parse_var(p)?);
                }
            }
        }
    }

    fn compile_text_node(
        p: &mut Parser,
    ) -> Result<(Option<Self>, CompileAfterTextNode), ParserError> {
        let text_node_start = p.current_char;
        let mut only_spaces = true;

        let gen_resp = |p: &mut Parser, only_spaces: bool| {
            if only_spaces {
                // We do not care about strings with only spaces
                None
            } else {
                let resp = SourceLocation(text_node_start, p.current_char - 1);
                if resp.is_empty() {
                    None
                } else {
                    Some(Self::Text(resp))
                }
            }
        };

        loop {
            match p.must_read_one()? {
                '<' => return Ok((gen_resp(p, only_spaces), CompileAfterTextNode::Tag)),
                '{' => {
                    if let Some(c) = p.seek_one() {
                        if c == '{' {
                            let resp = gen_resp(p, only_spaces);
                            p.current_char += 1;
                            return Ok((resp, CompileAfterTextNode::Var));
                        }
                    }
                }
                c if only_spaces && is_space(c) => {}
                _ => only_spaces = false,
            }
        }
    }

    fn parse_var(p: &mut Parser) -> Result<Self, ParserError> {
        let start = p.current_char;
        let global_vars = js::parse_template_var(p)?;
        let var = SourceLocation(start, p.current_char - 2);
        Ok(Self::Var(js::add_vm_references(p, &var, &global_vars)))
    }

    pub fn is_v_else_or_else_if(&self) -> bool {
        if let Child::Tag(tag, _) = self {
            if let Some(modifier) = tag.args.modifier.as_ref() {
                match modifier {
                    arg::VueTagModifier::ElseIf(_) | arg::VueTagModifier::Else => return true,
                    _ => {}
                }
            }
        }
        false
    }

    pub fn is_v_for(&self) -> bool {
        if let Child::Tag(tag, _) = self {
            if let Some(modifier) = tag.args.modifier.as_ref() {
                if let arg::VueTagModifier::For(_) = modifier {
                    return true;
                }
            }
        }
        false
    }
}

enum CompileAfterTextNode {
    Tag,
    Var,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub type_: TagType,
    pub name: SourceLocation,
    pub args: VueTagArgs,
    pub is_custom_component: bool,
}

pub fn is_tag_name_a_custom_component(parser: &Parser, tag_name: &SourceLocation) -> bool {
    let html_elements = vec![
        "a".chars(),
        "abbr".chars(),
        "acronym".chars(),
        "address".chars(),
        "applet".chars(),
        "area".chars(),
        "article".chars(),
        "aside".chars(),
        "audio".chars(),
        "b".chars(),
        "base".chars(),
        "basefont".chars(),
        "bdi".chars(),
        "bdo".chars(),
        "big".chars(),
        "blockquote".chars(),
        "body".chars(),
        "br".chars(),
        "button".chars(),
        "canvas".chars(),
        "caption".chars(),
        "center".chars(),
        "cite".chars(),
        "code".chars(),
        "col".chars(),
        "colgroup".chars(),
        "data".chars(),
        "datalist".chars(),
        "dd".chars(),
        "del".chars(),
        "details".chars(),
        "dfn".chars(),
        "dialog".chars(),
        "dir".chars(),
        "div".chars(),
        "dl".chars(),
        "dt".chars(),
        "em".chars(),
        "embed".chars(),
        "fieldset".chars(),
        "figcaption".chars(),
        "figure".chars(),
        "font".chars(),
        "footer".chars(),
        "form".chars(),
        "frame".chars(),
        "frameset".chars(),
        "head".chars(),
        "header".chars(),
        "hgroup".chars(),
        "h1".chars(),
        "h2".chars(),
        "h3".chars(),
        "h4".chars(),
        "h5".chars(),
        "h6".chars(),
        "hr".chars(),
        "html".chars(),
        "i".chars(),
        "iframe".chars(),
        "img".chars(),
        "input".chars(),
        "ins".chars(),
        "kbd".chars(),
        "keygen".chars(),
        "label".chars(),
        "legend".chars(),
        "li".chars(),
        "link".chars(),
        "main".chars(),
        "map".chars(),
        "mark".chars(),
        "menu".chars(),
        "menuitem".chars(),
        "meta".chars(),
        "meter".chars(),
        "nav".chars(),
        "noframes".chars(),
        "noscript".chars(),
        "object".chars(),
        "ol".chars(),
        "optgroup".chars(),
        "option".chars(),
        "output".chars(),
        "p".chars(),
        "param".chars(),
        "picture".chars(),
        "pre".chars(),
        "progress".chars(),
        "q".chars(),
        "rp".chars(),
        "rt".chars(),
        "ruby".chars(),
        "s".chars(),
        "samp".chars(),
        "script".chars(),
        "section".chars(),
        "select".chars(),
        "small".chars(),
        "source".chars(),
        "span".chars(),
        "strike".chars(),
        "strong".chars(),
        "style".chars(),
        "sub".chars(),
        "summary".chars(),
        "sup".chars(),
        "svg".chars(),
        "table".chars(),
        "tbody".chars(),
        "td".chars(),
        "template".chars(),
        "textarea".chars(),
        "tfoot".chars(),
        "th".chars(),
        "thead".chars(),
        "time".chars(),
        "title".chars(),
        "tr".chars(),
        "track".chars(),
        "tt".chars(),
        "u".chars(),
        "ul".chars(),
        "var".chars(),
        "video".chars(),
        "wbr".chars(),
    ];

    tag_name.eq_some(parser, false, html_elements).is_none()
}

// https://vuejs.org/v2/guide/render-function.html
// This is a somewhat rust representation of the vue component render arguments
#[derive(Debug, Clone)]
pub struct VueTagArgs {
    pub new_local_variables: Option<Vec<String>>,
    pub modifier: Option<arg::VueTagModifier>,
    pub has_js_component_args: bool,

    // Same API as `v-bind:class`, accepting either
    // a string, object, or array of strings and objects.
    // {foo: true, bar: false}
    pub class: Option<String>,

    // Same API as `v-bind:style`, accepting either
    // a string, object, or array of objects.
    //{ color: 'red', fontSize: '14px'}
    pub style: Option<String>,

    // Normal HTML attributes
    // OR
    // Component props
    // { foo: 'bar' }
    pub attrs_or_props: Option<Vec<(String, String)>>,

    // DOM properties
    // domProps: { innerHTML: 'baz' }
    pub dom_props: Option<Vec<(String, String)>>,

    // Event handlers are nested under `on`, though
    // modifiers such as in `v-on:keyup.enter` are not
    // supported. You'll have to manually check the
    // keyCode in the handler instead.
    // { click: this.clickHandler }
    pub on: Option<Vec<(String, String)>>,

    // For components only. Allows you to listen to
    // native events, rather than events emitted from
    // the component using `vm.$emit`.
    // nativeOn: { click: this.nativeClickHandler }
    pub native_on: Option<Vec<(String, String)>>,

    // Custom directives. Note that the `binding`'s
    // `oldValue` cannot be set, as Vue keeps track
    // of it for you.
    pub directives: Option<Vec<(String, String)>>,

    // TODO
    // Scoped slots in the form of
    // { name: props => VNode | Array<VNode> }
    // scopedSlots: {
    //   default: props => createElement('span', props.text)
    // },

    // The name of the slot, if this component is the
    // child of another component
    pub slot: Option<String>, // "name-of-slot"

    // Other special top-level properties
    // "myKey"
    pub key: Option<String>,
    // ref = "myRef"
    pub ref_: Option<String>,

    // If you are applying the same ref name to multiple
    // elements in the render function. This will make `$refs.myRef` become an array
    // refInFor = true
    pub ref_in_for: Option<bool>,
}

impl VueTagArgs {
    fn new() -> Self {
        Self {
            new_local_variables: None,
            modifier: None,
            has_js_component_args: false,
            class: None,
            style: None,
            attrs_or_props: None,
            dom_props: None,
            on: None,
            native_on: None,
            directives: None,
            slot: None,
            key: None,
            ref_: None,
            ref_in_for: None,
        }
    }

    pub fn has_attr_or_prop(&self, name: &str) -> Option<&str> {
        if let Some(attrs_or_props) = self.attrs_or_props.as_ref() {
            for (key, js_value) in attrs_or_props {
                if key == name {
                    return Some(&js_value);
                }
            }
        }
        None
    }

    pub fn has_attr_or_prop_with_string(&self, name: &str) -> Option<String> {
        let mut value = self.has_attr_or_prop(name)?.chars();

        let quote = match value.next()? {
            '\'' => '\'',
            '"' => '"',
            '`' => '`',
            _ => return None,
        };

        let mut resp = String::new();
        loop {
            match value.next()? {
                c if c == quote => break,
                '\\' => resp.push(value.next()?),
                c => resp.push(c),
            }
        }

        Some(resp)
    }

    fn add(
        &mut self,
        kind: arg::VueArgKind,
        key: String,
        value_as_js: String,
        is_custom_component: bool,
    ) -> Result<(), ParserError> {
        let set_has_js_component_args = match kind {
            arg::VueArgKind::Default | arg::VueArgKind::Bind => {
                match key.as_str() {
                    "class" => self.class = Some(value_as_js),
                    "style" => self.style = Some(value_as_js),
                    "slot" => self.slot = Some(value_as_js),
                    "key" => self.key = Some(value_as_js),
                    "ref" => self.ref_ = Some(value_as_js),
                    _ => add_or_set(&mut self.attrs_or_props, (key, value_as_js)),
                };
                true
            }
            arg::VueArgKind::On => {
                add_or_set(&mut self.on, (key, value_as_js));
                true
            }
            arg::VueArgKind::Text => {
                add_or_set(
                    &mut self.dom_props,
                    (String::from("textContent"), value_as_js),
                );
                true
            }
            arg::VueArgKind::Html => {
                add_or_set(
                    &mut self.dom_props,
                    (String::from("innerHTML"), value_as_js),
                );
                true
            }
            arg::VueArgKind::If => {
                self.set_modifier(arg::VueTagModifier::If(value_as_js))?;
                false
            }
            arg::VueArgKind::Else => {
                self.set_modifier(arg::VueTagModifier::Else)?;
                false
            }
            arg::VueArgKind::ElseIf => {
                self.set_modifier(arg::VueTagModifier::ElseIf(value_as_js))?;
                false
            }
            arg::VueArgKind::For => {
                panic!("this should never be called, it's a special value");
                // false
            }
            arg::VueArgKind::Model => {
                add_or_set(
                    &mut self.on,
                    (
                        String::from("input"),
                        format!(
                            "$event.target.composing?undefined:{}=$event.target.value",
                            &value_as_js
                        ),
                    ),
                );

                if is_custom_component {
                    add_or_set(&mut self.attrs_or_props, (key, value_as_js.clone()));
                } else {
                    add_or_set(&mut self.dom_props, (key, value_as_js.clone()));
                }

                add_or_set(&mut self.directives, (String::from("model"), value_as_js));
                true
            }
            arg::VueArgKind::Slot => {
                todo!("Slot");
                // true
            }
            arg::VueArgKind::Pre => {
                todo!("Pre");
                // true
            }
            arg::VueArgKind::Cloak => {
                todo!("Cloak");
                // true
            }
            arg::VueArgKind::Once => {
                todo!("Once");
                // true
            }
            arg::VueArgKind::CustomDirective(directive) => {
                add_or_set(&mut self.directives, (directive, value_as_js));
                true
            }
        };
        if set_has_js_component_args {
            self.has_js_component_args = true;
        }
        Ok(())
    }
    fn set_modifier(&mut self, to: arg::VueTagModifier) -> Result<(), ParserError> {
        if let Some(already_set_modifier) = self.modifier.as_ref() {
            Err(ParserError::new(
                "VueTagArgs::add",
                format!(
                    "cannot set {} on a tag that also has {}",
                    to.kind(),
                    already_set_modifier.kind()
                ),
            ))
        } else {
            self.modifier = Some(to);
            Ok(())
        }
    }
}
