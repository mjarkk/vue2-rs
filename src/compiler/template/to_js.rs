use super::super::utils::write_str;
use super::super::{js, utils, Parser, SourceLocation};
use super::{arg::VueTagModifier, Child, VueTagArgs};
use std::slice::Iter;

/*
_c('div', [
    _c('h1', [
        _vm._v(\"It wurks \" + _vm._s(_vm.count) + \" !\")
    ]),
    _c('button', { on: { \"click\": $event => { _vm.count++ } } }, [_vm._v(\"+\")]),
    _c('button', { on: { \"click\": $event => { _vm.count-- } } }, [_vm._v(\"-\")]),
])
*/

const DEFAULT_CONF: &'static str = "
__vue_2_file_default_export__.render = function(c) {
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
            children_to_js(&template.content, p, resp);
        }
        _ => {
            resp.push('[');
            children_to_js(&template.content, p, resp);
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

        let artifacts = child_to_js(child, p, resp);
        inside_of_if = artifacts.opened_inline_if_else;
        if artifacts.is_v_for {
            add_magic_number = Some(if children.len() > 1 {
                2
            } else {
                if artifacts.is_custom_component {
                    1
                } else {
                    0
                }
            })
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
    pub is_custom_component: bool,
}

pub fn child_to_js(child: &Child, p: &Parser, resp: &mut Vec<char>) -> ChildToJsArtifacts {
    let mut artifacts = ChildToJsArtifacts {
        opened_inline_if_else: false,
        is_v_for: false,
        is_custom_component: false,
    };

    // TODO support html escape
    match child {
        Child::Tag(tag, children) => {
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

            // Writes:
            // _c('div', [_c(..), _c(..)])
            write_str("_c('", resp);
            tag.name.write_to_vec_escape(p, resp, '\'', '\\');
            resp.push('\'');
            artifacts.is_custom_component = tag.is_custom_component;
            if tag.args.has_js_component_args {
                resp.push(',');
                vue_tag_args_to_js(&tag.args, resp, artifacts.is_custom_component);
            }

            resp.push(',');
            let result = if children.len() == 1 && children.get(0).unwrap().is_v_for() {
                children_to_js(children, p, resp)
            } else {
                resp.push('[');
                let result = children_to_js(children, p, resp);
                resp.push(']');
                result
            };

            if let Some(magic_number) = result.add_magic_number {
                // When using v-for a magic number is added
                // TODO: find out what this magic number exacly is
                resp.push(',');
                write_str(&magic_number.to_string(), resp);
            }
            write_str(")", resp);

            if artifacts.opened_inline_if_else {
                resp.push(':');
            } else if artifacts.is_v_for {
                resp.push(')');
            }
        }
        Child::Text(location) => {
            // Writes:
            // _vm._v("foo bar")
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

pub fn vue_tag_args_to_js(args: &VueTagArgs, dest: &mut Vec<char>, is_custom_component: bool) {
    dest.push('{');
    let mut object_entries = CommaSeparatedEntries::new();

    if let Some(class) = args.class.as_ref() {
        object_entries.add(dest);
        write_str("class:", dest);
        write_str(&class, dest);
    }

    if let Some(style) = args.style.as_ref() {
        object_entries.add(dest);
        write_str("style:", dest);
        write_str(&style, dest);
    }

    if let Some(attrs) = args.attrs_or_props.as_ref() {
        object_entries.add(dest);
        if is_custom_component {
            write_str("props:{", dest);
        } else {
            write_str("attrs:{", dest);
        }
        let mut attrs_entries = CommaSeparatedEntries::new();

        for (key, value) in attrs {
            attrs_entries.add(dest);

            dest.push('"');
            write_str(&js::escape_quotes(key, '"'), dest);
            write_str("\":", dest);

            for c in value.chars() {
                dest.push(c);
            }
        }

        dest.push('}');
    }

    if let Some(dom_props) = args.dom_props.as_ref() {
        object_entries.add(dest);
        write_str("domProps:{", dest);
        let mut dom_props_entries = CommaSeparatedEntries::new();

        for (key, value) in dom_props {
            dom_props_entries.add(dest);

            dest.push('"');
            write_str(&js::escape_quotes(key, '"'), dest);
            write_str("\":", dest);

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

            dest.push('"');
            write_str(&js::escape_quotes(key, '"'), dest);
            write_str("\":$event=>{", dest);

            for c in value.chars() {
                dest.push(c);
            }

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

            dest.push('"');
            write_str(&js::escape_quotes(key, '"'), dest);
            write_str("\":$event=>{", dest);

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

            write_str(",expression:\"", dest);
            write_str(&js::escape_quotes(&value, '"'), dest);
            dest.push('"');

            if let Some(target) = name.target.as_ref() {
                write_str(",arg:\"", dest);
                write_str(&js::escape_quotes(target, '"'), dest);
                dest.push('"');
            }

            if let Some(modifiers) = name.modifiers.as_ref() {
                write_str(",modifiers:{", dest);
                for modifier in modifiers {
                    dest.push('"');
                    write_str(&js::escape_quotes(modifier, '"'), dest);
                    write_str("\":true,", dest);
                }
                dest.push('}');
            }

            write_str("}", dest);
        }

        dest.push(']');
    }

    if let Some(slot) = args.slot.as_ref() {
        object_entries.add(dest);
        write_str("slot:", dest);
        write_str(&slot, dest);
    }

    if let Some(key) = args.key.as_ref() {
        object_entries.add(dest);
        write_str("key:", dest);
        write_str(&key, dest);
    }

    if let Some(ref_) = args.ref_.as_ref() {
        object_entries.add(dest);
        write_str("ref:", dest);
        write_str(&ref_, dest);
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
