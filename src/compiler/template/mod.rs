mod arg;
pub mod to_js;

use super::utils::is_space;
use super::{js, Parser, ParserError, SourceLocation};

// parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
// TODO support upper case tag names
pub fn parse_tag(p: &mut Parser, v_else_allowed: bool) -> Result<Tag, ParserError> {
    // let mut tag = Tag {
    //     type_: TagType::Open(),
    //     name: SourceLocation(p.current_char, 0),
    //     args: VueTagArgs::new(),
    // };

    let mut is_close_tag = false;
    let mut c = p.must_seek_one()?;

    if c == '/' {
        p.current_char += 1;
        is_close_tag = true;
    } else if c == '!' {
        let start_char = p.current_char;

        p.current_char += 1;

        match p.must_read_one()? {
            'D' => {
                let mut matches_doctype = true;
                let mut doctype = /*D*/"OCTYPE ".chars();
                while let Some(doctype_c) = doctype.next() {
                    let c = p.must_read_one()?;
                    if doctype_c != c {
                        matches_doctype = false;
                        break;
                    }
                }

                if matches_doctype {
                    p.look_for(vec!['>'])?;

                    return Ok(Tag {
                        type_: TagType::DocType,
                        name: SourceLocation::empty(),
                        args: VueTagArgs::new(),
                    });
                }
            }
            '-' => {
                if p.must_read_one()? == '-' {
                    p.look_for(vec!['-', '-', '>'])?;

                    return Ok(Tag {
                        type_: TagType::Comment,
                        name: SourceLocation::empty(),
                        args: VueTagArgs::new(),
                    });
                }
            }
            _ => {}
        }

        p.current_char = start_char;
        return Err(ParserError::new(
            p,
            format!("expect html comment (<!-- .. -->) or doctype (<!DOCTYPE ..>)"),
        ));
    }

    // Parse name
    let mut name = SourceLocation(p.current_char, 0);
    loop {
        match p.must_read_one()? {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => {}
            _ => {
                p.current_char -= 1;
                name.1 = p.current_char;
                break;
            }
        };
    }
    if name.1 == 0 {
        return Err(ParserError::new(p, "expected tag name"));
    }

    if is_close_tag {
        // We don't parse args for closing tags, look for the tag closing identifier (>)
        c = p.must_read_one_skip_spacing()?;
        if c != '>' {
            return Err(ParserError::new(
                p,
                format!("expected > but got '{}'", c.to_string()),
            ));
        }
        return Ok(Tag {
            type_: TagType::Close,
            name: name,
            args: VueTagArgs::new(),
        });
    }

    let kind = tag_name_kind(p, &name);
    let mut args = VueTagArgs::new();

    // Parse args
    loop {
        c = p.must_read_one_skip_spacing()?;
        c = match arg::try_parse(p, c, &mut args, v_else_allowed, &kind)? {
            Some(next_char) => next_char,
            None => c,
        };

        match c {
            '/' => {
                if is_close_tag {
                    return Err(ParserError::new(
                        p,
                        "/ not allowed after name in closing tag",
                    ));
                }
                c = p.must_read_one()?;
                if c != '>' {
                    return Err(ParserError::new(
                        p,
                        format!("expected > but got '{}'", c.to_string()),
                    ));
                }
                return Ok(Tag {
                    type_: TagType::OpenAndClose(kind),
                    name: name,
                    args: args,
                });
            }
            '>' => {
                return Ok(Tag {
                    type_: TagType::Open(kind),
                    name: name,
                    args: args,
                })
            }
            c if is_space(c) => {} // Ignore
            c => {
                return Err(ParserError::new(
                    p,
                    format!("unexpected character '{}'", c.to_string()),
                ))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum TagType {
    DocType,
    Comment,
    Open(TagKind),
    OpenAndClose(TagKind),
    Close,
}

impl TagType {
    pub fn to_string(&self) -> &'static str {
        match self {
            Self::DocType => "DOCTYPE",
            Self::Comment => "<!-- .. -->",
            Self::Open(_) => "open",
            Self::OpenAndClose(_) => "inline",
            Self::Close => "close",
        }
    }
}

pub fn add_or_set<T>(list: &mut Option<Vec<T>>, add: T) {
    if let Some(list) = list.as_mut() {
        list.push(add);
    } else {
        *list = Some(vec![add]);
    }
}

pub fn compile(p: &mut Parser) -> Result<Vec<Child>, ParserError> {
    let mut compile_result = Child::parse_children(p, &mut Vec::new())?;
    loop {
        if compile_result.closing_tag_name.eq(p, "template".chars()) {
            return Ok(compile_result.children);
        } else {
            let mut next_compile_result = Child::parse_children(p, &mut Vec::new())?;
            compile_result
                .children
                .append(&mut next_compile_result.children);
        }
    }
}

#[derive(Debug, Clone)]
pub enum Child {
    Tag(Tag, Vec<Child>),
    Text(SourceLocation),
    Var(String),
}

struct ParseChildrenResult {
    closing_tag_name: SourceLocation,
    children: Vec<Child>,
    children_with_v_slot: usize,
}

impl Child {
    // Returns a list of children, and the tag name of the closing tag
    fn parse_children(
        p: &mut Parser,
        parents_tag_names: &mut Vec<SourceLocation>,
    ) -> Result<ParseChildrenResult, ParserError> {
        let mut resp: Vec<Child> = Vec::with_capacity(1);
        let mut inside_v_if = false;
        let mut children_with_v_slot = 0usize;

        loop {
            let (text_node, compile_now) = Self::compile_text_node(p)?;
            if let Some(node) = text_node {
                inside_v_if = false;
                resp.push(node);
            }

            match compile_now {
                CompileAfterTextNode::Tag => {
                    let mut tag = parse_tag(p, inside_v_if)?;

                    if let Some(modifier) = tag.args.modifier.as_ref() {
                        inside_v_if = match modifier {
                            arg::VueTagModifier::If(_) => true,
                            arg::VueTagModifier::ElseIf(_) => true,
                            _ => false,
                        };
                    } else {
                        inside_v_if = false;
                    }

                    if tag.args.slot.is_some() {
                        children_with_v_slot += 1;
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

                            if found {
                                return Ok(ParseChildrenResult {
                                    closing_tag_name: tag.name,
                                    children: resp,
                                    children_with_v_slot,
                                });
                            }

                            // Always go back in the tree if a template tag is found
                            if tag.name.eq(p, "template".chars()) {
                                return Ok(ParseChildrenResult {
                                    closing_tag_name: tag.name,
                                    children: resp,
                                    children_with_v_slot,
                                });
                            }
                        }
                        TagType::Open(_) => {
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
                            let compiled_children = compile_children_result?;

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

                            if compiled_children.children_with_v_slot > 0 {
                                tag.args.has_js_component_args = true;
                                tag.args.children_with_slot =
                                    compiled_children.children_with_v_slot;
                            }

                            resp.push(Self::Tag(tag, compiled_children.children));

                            let correct_closing_tag =
                                tag_name.eq_self(p, &compiled_children.closing_tag_name);
                            if !correct_closing_tag {
                                return Ok(ParseChildrenResult {
                                    children: resp,
                                    closing_tag_name: compiled_children.closing_tag_name,
                                    children_with_v_slot,
                                });
                            }
                        }
                        TagType::OpenAndClose(_) => {
                            resp.push(Self::Tag(tag, Vec::new()));
                        }
                        TagType::Comment | TagType::DocType => {} // Skip these tag
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
}

#[derive(Debug, Clone)]
pub enum TagKind {
    HtmlElement,
    CustomComponent,
    Slot,
}

pub fn tag_name_kind(parser: &Parser, tag_name: &SourceLocation) -> TagKind {
    let slot_and_html_elements = vec![
        // Check for slot
        "slot".chars(),
        //
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

    match tag_name.eq_some(parser, false, slot_and_html_elements) {
        Some(0) => TagKind::Slot,
        Some(_) => TagKind::HtmlElement,
        None => TagKind::CustomComponent,
    }
}

#[derive(Debug, Clone)]
pub enum StaticOrJS {
    Non,
    Static(String),
    Bind(String),
}

impl StaticOrJS {
    fn is_static(&self) -> Option<&str> {
        if let Self::Static(v) = self {
            Some(v.as_str())
        } else {
            None
        }
    }
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
    pub class: Option<StaticOrJS>,

    // Same API as `v-bind:style`, accepting either
    // a string, object, or array of objects.
    //{ color: 'red', fontSize: '14px'}
    pub style: Option<StaticOrJS>,

    // Normal HTML attributes
    // OR
    // Component props
    // { foo: 'bar' }
    pub attrs_or_props: Option<Vec<(String, StaticOrJS)>>,

    // DOM properties
    // domProps: { innerHTML: 'baz' }
    // The value is expected to be JS
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
    pub directives: Option<Vec<(arg::ParseArgNameResult, String)>>,

    // A template tag might have a v-slot attribute
    pub slot: Option<(String, Option<String>)>,
    pub children_with_slot: usize,

    // Other special top-level properties
    // "myKey"
    pub key: Option<StaticOrJS>,
    // ref = "myRef"
    pub ref_: Option<StaticOrJS>,

    // If you are applying the same ref name to multiple
    // elements in the render function. This will make `$refs.myRef` become an array
    // refInFor = true
    pub ref_in_for: Option<bool>,

    // slot_v_bind is a special operator for slot tags using v-bind
    // (<slot v-bind="{foo: 'bar'}" />)
    pub slot_v_bind: Option<StaticOrJS>,

    // Contains the name attribute value in case of a <slot ..> tag
    pub slot_tag_name_attr: Option<StaticOrJS>,
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
            children_with_slot: 0,
            key: None,
            ref_: None,
            ref_in_for: None,
            slot_v_bind: None,
            slot_tag_name_attr: None,
        }
    }

    pub fn has_attr_or_prop(&self, name: &str) -> Option<&StaticOrJS> {
        if let Some(attrs_or_props) = self.attrs_or_props.as_ref() {
            for (key, value) in attrs_or_props {
                if key == name {
                    return Some(value);
                }
            }
        }
        None
    }

    pub fn has_attr_or_prop_with_string(&self, name: &str) -> Option<&str> {
        Some(self.has_attr_or_prop(name)?.is_static()?)
    }

    fn set_default_or_bind(&mut self, key: &str, value: StaticOrJS) -> Result<(), ParserError> {
        match key {
            "class" => self.class = Some(value),
            "style" => self.style = Some(value),
            "key" => self.key = Some(value),
            "ref" => self.ref_ = Some(value),
            _ => add_or_set(&mut self.attrs_or_props, (key.to_string(), value)),
        };
        Ok(())
    }
    fn set_slot_v_bind(&mut self, p: &Parser, to: StaticOrJS) -> Result<(), ParserError> {
        if self.slot_v_bind.is_some() {
            return Err(ParserError::new(p, "cannot set v-bind twice"));
        }
        self.slot_v_bind = Some(to);
        Ok(())
    }
    fn set_slot_tag_name_attr(&mut self, p: &Parser, to: StaticOrJS) -> Result<(), ParserError> {
        if self.slot_tag_name_attr.is_some() {
            return Err(ParserError::new(p, "cannot set v-bind twice"));
        }
        self.slot_tag_name_attr = Some(to);
        Ok(())
    }
    fn set_modifier(&mut self, p: &Parser, to: arg::VueTagModifier) -> Result<(), ParserError> {
        if let Some(already_set_modifier) = self.modifier.as_ref() {
            Err(ParserError::new(
                p,
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
