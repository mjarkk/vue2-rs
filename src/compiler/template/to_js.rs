use super::super::utils::write_str;
use super::super::{utils, Parser, SourceLocation};
use super::{arg::VueTagModifier, Child, StaticOrJS, VueTagArgs};
use super::{TagKind, TagType};
use std::slice::Iter;

/*
    TODO support html escape
*/

const DEFAULT_CONF: &'static str = "
c._compiled = true;
c.staticRenderFns = [];
c.render = function(c) {
    const _vm = this;
    const _h = _vm.$createElement;
    const _c = _vm._self._c || _h;
    return ";

pub fn template_to_js(p: &Parser, resp: &mut Vec<char>) {
    let template = match p.template.as_ref() {
        Some(t) => t,
        None => return,
    };

    resp.append(&mut DEFAULT_CONF.chars().collect());

    match template.content.len() {
        0 => {
            write_str("[]", resp);
        }
        1 => {
            children_to_js(&template.content, p, resp, false);
        }
        _ => {
            resp.push('[');
            children_to_js(&template.content, p, resp, false);
            resp.push(']');
        }
    }

    resp.append(&mut "\n};".chars().collect())
}

pub struct AddChildrenResult {
    pub add_magic_number: Option<u8>,
}

pub fn children_to_js(
    children: &Vec<Child>,
    p: &Parser,
    resp: &mut Vec<char>,
    filter_out_tags_with_slot_attr: bool,
) -> AddChildrenResult {
    let mut list_builder = CommaSeparatedEntries::new();
    let mut children_iter = children.iter();
    let mut inside_of_if = false;

    let mut add_magic_number: Option<u8> = None;

    while let Some(mut child) = children_iter.next() {
        if !inside_of_if {
            list_builder.add(resp);
        } else if !child.is_v_else_or_else_if() {
            // We are at the end of a v-if > v-else-if
            // There is no else case written yet, lets write that here
            write_str("_vm._e()", resp);
            list_builder.add(resp);
        }

        match child {
            // Writes:
            // _vm._v("foo bar")
            // Or in case of text mixed with vars:
            // _vm._v("foo bar " + _vm._s(_vm.some_var) + "!")
            Child::Var(var) => {
                write_str("_vm._v(", resp);
                write_vue_js_var(var, resp);
                let might_next_child = concat_next_text_and_vars(p, &mut children_iter, resp);
                resp.push(')');

                if let Some(next_child) = might_next_child {
                    child = next_child;
                    list_builder.add(resp);
                } else {
                    break;
                }
            }
            Child::Text(location) => {
                write_str("_vm._v(", resp);
                write_text_quote(p, location, resp);
                let might_next_child = concat_next_text_and_vars(p, &mut children_iter, resp);
                resp.push(')');

                if let Some(next_child) = might_next_child {
                    child = next_child;
                    list_builder.add(resp);
                } else {
                    break;
                }
            }
            _ => {}
        }

        let artifacts = child_to_js(child, p, resp, !filter_out_tags_with_slot_attr);

        if artifacts.skipped {
            continue;
        }

        inside_of_if = artifacts.opened_inline_if_else;
        if let Some(remaining_v_for_magic_number) = artifacts.move_v_for_magic_number_up {
            add_magic_number = Some(if children.len() > 1 {
                2
            } else {
                remaining_v_for_magic_number
            });
        } else if artifacts.is_slot {
            add_magic_number = Some(2);
        } else if artifacts.is_v_for {
            add_magic_number = Some(if children.len() > 1 {
                2
            } else {
                if artifacts.is_custom_component {
                    1
                } else {
                    0
                }
            });
        }
    }

    if inside_of_if {
        write_str("_vm._e()", resp);
    }

    AddChildrenResult { add_magic_number }
}

fn concat_next_text_and_vars<'a>(
    p: &Parser,
    children_iter: &mut Iter<'a, Child>,
    resp: &mut Vec<char>,
) -> Option<&'a Child> {
    loop {
        if let Some(child) = children_iter.next() {
            match child {
                Child::Text(location) => {
                    resp.push('+');
                    write_text_quote(p, location, resp);
                }
                Child::Var(var) => {
                    resp.push('+');
                    write_vue_js_var(var, resp);
                }
                Child::Tag(_, _) => {
                    return Some(child);
                }
            }
        } else {
            return None;
        }
    }
}

#[derive(Debug)]
pub struct ChildToJsArtifacts {
    pub opened_inline_if_else: bool,
    pub is_v_for: bool,
    pub move_v_for_magic_number_up: Option<u8>,
    pub is_custom_component: bool,
    pub is_slot: bool,
    pub skipped: bool,
}

pub fn child_to_js(
    child: &Child,
    p: &Parser,
    resp: &mut Vec<char>,
    slot_attr_allowed: bool,
) -> ChildToJsArtifacts {
    let mut artifacts = ChildToJsArtifacts {
        opened_inline_if_else: false,
        is_v_for: false,
        move_v_for_magic_number_up: None,
        is_custom_component: false,
        is_slot: false,
        skipped: false,
    };

    match child {
        Child::Tag(tag, children) => {
            if !slot_attr_allowed && tag.args.slot.is_some() {
                artifacts.skipped = true;
                return artifacts;
            }

            if let Some(modifier) = tag.args.modifier.as_ref() {
                match modifier {
                    VueTagModifier::For(for_args) => {
                        artifacts.is_v_for = true;
                        write_str("_vm._l((", resp);
                        write_str(&for_args.list, resp);
                        write_str("),(", resp);
                        write_str(&for_args.value, resp);
                        if let Some(key) = for_args.key.as_ref() {
                            resp.push(',');
                            write_str(key, resp);
                            if let Some(index) = for_args.index.as_ref() {
                                resp.push(',');
                                write_str(index, resp);
                            }
                        }
                        write_str(")=>", resp);
                    }
                    VueTagModifier::If(js_check) => {
                        artifacts.opened_inline_if_else = true;
                        write_str(&js_check, resp);
                        resp.push('?');
                    }
                    VueTagModifier::ElseIf(js_check) => {
                        artifacts.opened_inline_if_else = true;
                        write_str(&js_check, resp);
                        resp.push('?');
                    }
                    VueTagModifier::Else => {} // Do nothing
                }
            }

            let mut children_len = children.len();

            let custom_tag_check =
                tag.name
                    .eq_some(p, false, vec!["template".chars(), "slot".chars()]);

            match custom_tag_check {
                Some(0) => {
                    // Is <template>
                    if children_len == 0 {
                        write_str("void 0", resp);
                    } else {
                        let result = if children_len == 1 && children.get(0).unwrap().is_v_for() {
                            children_to_js(children, p, resp, false)
                        } else {
                            resp.push('[');
                            let result = children_to_js(children, p, resp, false);
                            resp.push(']');
                            result
                        };

                        if let Some(magic_number) = result.add_magic_number {
                            artifacts.move_v_for_magic_number_up = Some(magic_number);
                            artifacts.is_v_for = true;
                        }
                    }
                }
                Some(1) => {
                    // Is <slot>
                    artifacts.is_slot = true;

                    if let Some(name) = tag.args.slot_tag_name_attr.as_ref() {
                        write_str("_vm._t(", resp);
                        write_static_or_js(name, resp);
                    } else {
                        write_str("_vm._t(\"default\"", resp);
                    }
                    if children_len != 0 {
                        write_str(",function(){return [", resp);
                        children_to_js(children, p, resp, false);
                        write_str("]}", resp);
                    } else if tag.args.attrs_or_props.is_some() || tag.args.slot_v_bind.is_some() {
                        write_str(",null", resp);
                    }
                    if let Some(props) = tag.args.attrs_or_props.as_ref() {
                        write_str(",", resp);
                        write_object(props, resp);
                    }
                    if let Some(data) = tag.args.slot_v_bind.as_ref() {
                        resp.push(',');
                        write_static_or_js(data, resp);
                    }
                    resp.push(')');
                }
                _ => {
                    // Is a normal tag,
                    //
                    // Writes:
                    // _c('div', [_c(..), _c(..)])

                    children_len -= tag.args.children_with_slot;

                    write_str("_c('", resp);
                    tag.name.write_to_vec_escape(p, resp, '\'', '\\');
                    resp.push('\'');
                    artifacts.is_custom_component = match &tag.type_ {
                        TagType::Open(kind) | TagType::OpenAndClose(kind) => match kind {
                            TagKind::Slot => true,
                            TagKind::CustomComponent => true,
                            TagKind::HtmlElement => false,
                        },
                        _ => true,
                    };
                    if tag.args.has_js_component_args {
                        resp.push(',');
                        vue_tag_args_to_js(
                            children,
                            &tag.args,
                            resp,
                            artifacts.is_custom_component,
                            p,
                        );
                    }

                    if children_len != 0 {
                        resp.push(',');
                        let result = if children_len == 1 && children.get(0).unwrap().is_v_for() {
                            children_to_js(children, p, resp, true)
                        } else {
                            resp.push('[');
                            let result = children_to_js(children, p, resp, true);
                            resp.push(']');
                            result
                        };

                        if let Some(magic_number) = result.add_magic_number {
                            // When using v-for a magic number is added
                            // TODO: find out what this magic number exactly is
                            resp.push(',');
                            write_str(&magic_number.to_string(), resp);
                        }
                    }
                    resp.push(')');
                }
            };

            if artifacts.opened_inline_if_else {
                resp.push(':');
            } else if artifacts.is_v_for {
                resp.push(')');
            }
        }
        Child::Text(location) => {
            write_str("_vm._v(", resp);
            write_text_quote(p, location, resp);
            resp.push(')');
        }
        Child::Var(var) => {
            write_vue_js_var(var, resp);
        }
    };

    artifacts
}

fn write_vue_js_var(var: &str, resp: &mut Vec<char>) {
    // Writes _vm._s(_vm.some_var)
    write_str("_vm._s(", resp);
    write_str(&var, resp);
    resp.push(')');
}

fn write_text_quote(p: &Parser, location: &SourceLocation, resp: &mut Vec<char>) {
    resp.push('"');

    let mut chars_iter = location.chars(p).iter();
    'outer_loop: while let Some(c_ref) = chars_iter.next() {
        let mut c = *c_ref;
        if utils::is_space(c) {
            resp.push(' ');
            loop {
                if let Some(next_c_ref) = chars_iter.next() {
                    if !utils::is_space(*next_c_ref) {
                        c = *next_c_ref;
                        break;
                    }
                } else {
                    break 'outer_loop;
                }
            }
        }

        if c == '"' || c == '\\' {
            resp.push('\\');
        }
        resp.push(c);
    }

    resp.push('"');
}

pub fn vue_tag_args_to_js(
    children: &Vec<Child>,
    args: &VueTagArgs,
    dest: &mut Vec<char>,
    is_custom_component: bool,
    p: &Parser,
) {
    dest.push('{');
    let mut object_entries = CommaSeparatedEntries::new();

    if let Some(class) = args.class.as_ref() {
        match class {
            StaticOrJS::Non => {}
            StaticOrJS::Static(value) => {
                object_entries.add(dest);
                write_str("staticClass:", dest);
                write_str_with_quotes(value, dest);
            }
            StaticOrJS::Bind(value) => {
                object_entries.add(dest);
                write_str("class:", dest);
                write_str(&value, dest);
            }
        };
    }

    if let Some(style) = args.style.as_ref() {
        match style {
            StaticOrJS::Non => {}
            StaticOrJS::Static(value) => {
                object_entries.add(dest);
                write_str("style:", dest);
                write_str_with_quotes(value, dest);
            }
            StaticOrJS::Bind(value) => {
                object_entries.add(dest);
                write_str("style:", dest);
                write_str(&value, dest);
            }
        }
    }

    if let Some(attrs) = args.attrs_or_props.as_ref() {
        object_entries.add(dest);
        if is_custom_component {
            write_str("props:", dest);
        } else {
            write_str("attrs:", dest);
        }
        write_object(attrs, dest);
    }

    if let Some(dom_props) = args.dom_props.as_ref() {
        object_entries.add(dest);
        write_str("domProps:{", dest);
        let mut dom_props_entries = CommaSeparatedEntries::new();

        for (key, value) in dom_props {
            dom_props_entries.add(dest);

            write_str_with_quotes(key, dest);
            dest.push(':');

            for c in value.chars() {
                dest.push(c);
            }
        }

        dest.push('}');
    }

    if let Some(on) = args.on.as_ref() {
        object_entries.add(dest);
        write_str("on:{", dest);
        let mut on_entries = CommaSeparatedEntries::new();

        for (key, value) in on {
            on_entries.add(dest);

            write_str_with_quotes(key, dest);
            write_str(":$event=>{", dest);
            write_str(&value, dest);

            dest.push('}');
        }

        dest.push('}');
    }

    if let Some(on) = args.native_on.as_ref() {
        object_entries.add(dest);
        write_str("nativeOn:{", dest);
        let mut on_entries = CommaSeparatedEntries::new();

        for (key, value) in on {
            on_entries.add(dest);

            write_str_with_quotes(key, dest);
            write_str(":$event=>{", dest);

            for c in value.chars() {
                dest.push(c);
            }

            dest.push('}');
        }

        dest.push('}');
    }

    if let Some(directives) = args.directives.as_ref() {
        object_entries.add(dest);
        write_str("directives:[", dest);
        let mut directive_entries = CommaSeparatedEntries::new();

        for (name, value) in directives {
            directive_entries.add(dest);
            write_str("{name:\"", dest);
            write_str(name.name.split_at(2).1, dest);
            dest.push('"');

            write_str(",rawName:\"", dest);
            write_str(&name.name, dest);
            if let Some(target) = name.target.as_ref() {
                dest.push(':');
                write_str(target, dest);
            }
            if let Some(modifiers) = name.modifiers.as_ref() {
                for modifier in modifiers {
                    dest.push('.');
                    write_str(modifier, dest);
                }
            }
            dest.push('"');

            write_str(",value:", dest);
            write_str(value, dest);

            write_str(",expression:", dest);
            write_str_with_quotes(&value, dest);

            if let Some(target) = name.target.as_ref() {
                write_str(",arg:", dest);
                write_str_with_quotes(target, dest);
            }

            if let Some(modifiers) = name.modifiers.as_ref() {
                write_str(",modifiers:{", dest);
                for modifier in modifiers {
                    write_str_with_quotes(modifier, dest);
                    write_str(":true,", dest);
                }
                dest.push('}');
            }

            write_str("}", dest);
        }

        dest.push(']');
    }

    if args.children_with_slot > 0 {
        object_entries.add(dest);
        write_str("scopedSlots:_vm._u([", dest);
        let mut scoped_slots_entries = CommaSeparatedEntries::new();
        for child in children {
            if let Child::Tag(v, children) = child {
                if let Some((slot_name, _)) = v.args.slot.as_ref() {
                    scoped_slots_entries.add(dest);
                    // Writes:
                    // {key:"test",fn:function(){return [_c("div", [_vm._v("Test Slot content")])];},proxy:true}

                    write_str("{key:", dest);
                    write_str_with_quotes(slot_name, dest);
                    write_str(",fn:function(){return [", dest);
                    children_to_js(children, p, dest, true);
                    write_str("]},proxy:true}", dest);
                }
            }
        }
        write_str("])", dest);
    }

    if let Some(key) = args.key.as_ref() {
        object_entries.add(dest);
        write_str("key:", dest);
        write_static_or_js(key, dest);
    }

    if let Some(ref_) = args.ref_.as_ref() {
        object_entries.add(dest);
        write_str("ref:", dest);
        write_static_or_js(ref_, dest);
    }

    if let Some(ref_in_for) = args.ref_in_for.as_ref() {
        object_entries.add(dest);
        if *ref_in_for {
            write_str("refInFor:true", dest);
        } else {
            write_str("refInFor:false", dest);
        }
    }

    dest.push('}');
}

struct CommaSeparatedEntries(bool);

impl CommaSeparatedEntries {
    fn new() -> Self {
        Self(false)
    }
    fn add(&mut self, dest: &mut Vec<char>) {
        if self.0 {
            dest.push(',');
        } else {
            self.0 = true;
        }
    }
}

fn write_str_with_quotes(value: &str, dest: &mut Vec<char>) {
    dest.push('"');
    utils::write_str_escaped(value, '"', '\\', dest);
    dest.push('"');
}

fn write_static_or_js(value: &StaticOrJS, dest: &mut Vec<char>) {
    match value {
        StaticOrJS::Non => write_str("true", dest),
        StaticOrJS::Bind(v) => write_str(&v, dest),
        StaticOrJS::Static(v) => write_str_with_quotes(&v, dest),
    }
}

fn write_object(key_values: &Vec<(String, StaticOrJS)>, dest: &mut Vec<char>) {
    dest.push('{');
    let mut entries = CommaSeparatedEntries::new();
    for (key, value) in key_values {
        entries.add(dest);
        write_str_with_quotes(key, dest);
        dest.push(':');
        write_static_or_js(value, dest);
    }
    dest.push('}');
}
