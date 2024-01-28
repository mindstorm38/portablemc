from io import StringIO
from argparse import ArgumentParser, \
    _CountAction, _StoreAction, _SubParsersAction, \
    _StoreConstAction, _HelpAction, _AppendConstAction

from .lang import get as _
from .parse import type_path, type_path_dir, \
    type_email_or_username, type_host, get_completions

from typing import Dict, Tuple, cast


def escape_zsh(s: str, *, space = False) -> str:
    s = s.replace("'", "''").replace("[", "\\[").replace("]", "\\]").replace(":", "\\:")
    if space:
        s = s.replace(" ", "\\ ")
    return s


def gen_zsh_completion(parser: ArgumentParser) -> str:
    buffer = StringIO()
    gen_zsh_parser_completion(parser, buffer, "_complete_portablemc")
    buffer.write("compdef _complete_portablemc portablemc\n")
    return buffer.getvalue()


def gen_zsh_parser_completion(parser: ArgumentParser, buffer: StringIO, function: str):

    commands: Dict[str, Tuple[str, ArgumentParser]] = {}
    completions: Dict[str, Dict[str, str]] = {}

    buffer.write(function)
    buffer.write("() {\n")
    buffer.write("  local curcontext=$curcontext state line\n")
    buffer.write("  integer ret=1\n")

    buffer.write("  _arguments -s -C \\\n")

    for action in parser._actions:
        
        zsh_description = escape_zsh(action.help or "")
        zsh_repeat = ""
        zsh_action = ": :"

        # Depending on the action type there are some specific things we can do.
        if isinstance(action, _CountAction):
            zsh_repeat = "\\*"
            zsh_action = ""
        elif isinstance(action, _StoreAction):

            action_completions = get_completions(action)
            if action.choices is not None:
                for choice in action.choices:
                    if choice not in action_completions:
                        action_completions[choice] = ""
            
            if action.type == type_path:
                zsh_action = ": :_files"
            elif action.type == type_path_dir:
                zsh_action = ": :_files -/"
            elif action.type == type_email_or_username:
                zsh_action = ": :_email_addresses -c"
            elif action.type == type_host:
                zsh_action = ": :_hosts"
            elif len(action_completions):
                zsh_action = f": :->action_{action.dest}"
                completions[f"action_{action.dest}"] = action_completions

        elif isinstance(action, (_HelpAction, _StoreConstAction, _AppendConstAction)):
            zsh_action = ""
        elif isinstance(action, _SubParsersAction):
            parsers_choices = cast(Dict[str, ArgumentParser], action.choices)
            for sub_action in action._get_subactions():
                commands[sub_action.dest] = (sub_action.help or "", parsers_choices[sub_action.dest])
            continue

        # If the argument is positional.
        if not len(action.option_strings):
            buffer.write(f"    '{zsh_action}' \\\n")
            continue
        
        # If the argument is an option.
        if len(action.option_strings) > 1:
            zsh_names = f"{{{','.join(action.option_strings)}}}"
        else:
            zsh_names = action.option_strings[0]
        buffer.write(f"    {zsh_repeat}{zsh_names}'[{zsh_description}]{zsh_action}' \\\n")
    
    if len(commands):
        buffer.write("    ': :->command' \\\n")
        buffer.write("    '*:: :->option' \\\n")

    buffer.write("    && ret=0\n")

    if len(commands) or len(completions):

        buffer.write("  case $state in\n")

        if len(commands):
            buffer.write("  command)\n")
            buffer.write("    local -a commands=(\n")
            for name, (description, parser) in commands.items():
                buffer.write(f"      '{name}:{escape_zsh(description)}'\n")
            buffer.write("    )\n")
            buffer.write("    _describe -t commands command commands && ret=0\n")
            buffer.write("    ;;\n")

            buffer.write("  option)\n")
            buffer.write("    case $line[1] in\n")
            for name, (description, parser) in commands.items():
                buffer.write(f"    {name}) {function}_{name} ;;\n")
            buffer.write("    esac\n")
            buffer.write("    ;;\n")

        for state, action_completions in completions.items():
            buffer.write(f"  {state})\n")
            buffer.write("    local -a completions=(\n")
            for name, description in action_completions.items():
                if len(description):
                    buffer.write(f"      '{escape_zsh(name)}:{escape_zsh(description)}'\n")
                else:
                    buffer.write(f"      '{escape_zsh(name)}'\n")
            buffer.write("    )\n")
            buffer.write("    _describe -t values value completions && ret=0\n")
            buffer.write("    ;;\n")
        
        buffer.write("  esac\n")

    buffer.write("}\n\n")

    for name, (description, parser) in commands.items():
        gen_zsh_parser_completion(parser, buffer, f"{function}_{name}")
