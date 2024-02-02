from io import StringIO
from argparse import ArgumentParser, \
    _CountAction, _StoreAction, _SubParsersAction, \
    _StoreConstAction, _HelpAction, _AppendConstAction

from .lang import get as _
from .parse import type_path, type_path_dir, \
    type_email_or_username, type_host, get_completions

from typing import Dict, Tuple, cast


def gen_zsh_completion(parser: ArgumentParser) -> str:
    buffer = StringIO()
    buffer.write("#compdef portablemc\n\n")
    gen_zsh_parser_completion(parser, buffer, "_portablemc")
    buffer.write("if [[ $zsh_eval_context[-1] == loadautofunc ]]; then\n")
    buffer.write("  _portablemc\n")
    buffer.write("else\n")
    buffer.write("  compdef _portablemc portablemc\n")
    buffer.write("fi\n")
    return buffer.getvalue()

def gen_zsh_parser_completion(parser: ArgumentParser, buffer: StringIO, function: str):

    # Sources:
    # - https://zsh.sourceforge.io/Doc/Release/Completion-Widgets.html
    # - https://zsh.sourceforge.io/Doc/Release/Completion-System.html

    commands: Dict[str, Tuple[str, ArgumentParser]] = {}
    completions: Dict[str, Dict[str, str]] = {}

    buffer.write(function)
    buffer.write(" () {\n")
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
                zsh_action = f": :->arg_{action.dest}"
                completions[f"arg_{action.dest}"] = action_completions

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
            for name, (cmd_description, cmd_parser) in commands.items():
                buffer.write(f"      '{name}:{escape_zsh(cmd_description)}'\n")
            buffer.write("    )\n")
            buffer.write("    _describe -t commands command commands && ret=0\n")
            buffer.write("    ;;\n")

            buffer.write("  option)\n")
            buffer.write("    case $line[1] in\n")
            for name, (cmd_description, cmd_parser) in commands.items():
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

    for cmd_name, (cmd_description, cmd_parser) in commands.items():
        gen_zsh_parser_completion(cmd_parser, buffer, f"{function}_{cmd_name}")

def escape_zsh(s: str) -> str:
    return s.replace("'", "''").replace("[", "\\[").replace("]", "\\]").replace(":", "\\:")


def gen_bash_completion(parser: ArgumentParser) -> str:
    buffer = StringIO()
    buffer.write("#/usr/bin/env bash\n\n")
    gen_bash_parser_completion(parser, buffer, "_portablemc")
    buffer.write("\ncomplete -o filenames -o nosort -F _portablemc portablemc\n")
    return buffer.getvalue()

def gen_bash_parser_completion(parser: ArgumentParser, buffer: StringIO, function: str):

    # Note: We use single quote in this function because we double quote in bash.
    # Sources:
    # - https://www.gnu.org/software/bash/manual/html_node/Programmable-Completion-Builtins.html
    # - https://www.gnu.org/savannah-checkouts/gnu/bash/manual/bash.html
    #
    # Current limitations of the bash completer:
    # - Choices for positional arguments are not support, luckily the launcher don't use 
    #   such construct.
    # - Short arguments cannot be completed when stacked.

    buffer.write(function)
    buffer.write(' ()\n{\n')

    # We guess that there we always be two argument, the command and the argument to comp.
    buffer.write('  local index="$(( COMP_CWORD - 1 ))"\n')
    buffer.write('  local words=(${COMP_WORDS[@]:1})\n')
    buffer.write('  local word="${words[$index]}"\n')

    # Start by finding if there are sub parsers.
    commands: Dict[str, ArgumentParser] = {}
    for action in parser._actions:
        if isinstance(action, _SubParsersAction):
            commands.update(action.choices)
        elif len(action.option_strings):
            # Named argument
            buffer.write(f'  local arg_{action.dest}="{" ".join(action.option_strings)}"\n')
    
    # Write a loop to find potential sub-command, and construct arguments list.
    # We overwrite the COMP_ variables because we don't use them after loop.
    buffer.write('  for i in ${!words[@]}; do\n')

    # Start by sub-commands...
    if len(commands):
        buffer.write("    if (( i < index )); then\n")
        buffer.write("      COMP_WORDS=(${words[@]:$i})\n")
        buffer.write("      COMP_CWORD=$((index - i))\n")
        buffer.write('      case "${words[$i]}" in\n')
        for cmd_name, cmd_parser in commands.items():
            buffer.write(f'      {cmd_name}) {function}_{cmd_name}; return ;;\n')
        buffer.write("      esac\n")
        buffer.write("    fi\n")
    
    # Then arguments...
    buffer.write('    case "${words[$i]}" in\n')
    for action in parser._actions:
        if isinstance(action, (_SubParsersAction, _CountAction)):
            pass  # Count action are not limited in number
        elif len(action.option_strings):
            buffer.write( "    ")
            buffer.write( " | ".join(f'"{option}"' for option in action.option_strings))
            buffer.write(f') arg_{action.dest}="" ;;\n')
    buffer.write("    esac\n")

    buffer.write("  done\n")

    # Special case for options with associated value.
    buffer.write('  if (( index >= 1 )); then\n')
    buffer.write('    case ${words[$(( index - 1 ))]} in\n')
    
    for action in parser._actions:
        
        if isinstance(action, _StoreAction):

            if len(action.option_strings):
                
                buffer.write("    ")
                buffer.write(" | ".join(f'"{option}"' for option in action.option_strings))
                buffer.write(")\n")
                
                reply = ""

                if action.type == type_path:
                    reply = '$(compgen -o plusdirs -f -- "$word")'
                elif action.type == type_path_dir:
                    reply = '$(compgen -o plusdirs -d -- "$word")'
                elif action.type == type_email_or_username:
                    pass
                elif action.type == type_host:
                    reply = '$(compgen -A hostname -- "$word")'
                elif action.choices is not None:
                    reply = f'$(compgen -W "{" ".join(action.choices)}" -- "$word")'

                buffer.write(f"      COMPREPLY=({reply})\n")
                buffer.write( "      return\n")
                buffer.write( "      ;;\n")
            
    buffer.write("    esac\n")
    buffer.write("  fi\n")

    # This is the default reply for argument names.
    buffer.write('  COMPREPLY=($(compgen -W "')
    for cmd_name, cmd_parser in commands.items():
        buffer.write(f"{cmd_name} ")
    for action in parser._actions:
        if isinstance(action, _SubParsersAction):
            pass
        elif len(action.option_strings):
            buffer.write(f"$arg_{action.dest} ")
    buffer.write('" -- "$word"))\n')

    buffer.write('}\n\n')

    for cmd_name, cmd_parser in commands.items():
        gen_bash_parser_completion(cmd_parser, buffer, f"{function}_{cmd_name}")
