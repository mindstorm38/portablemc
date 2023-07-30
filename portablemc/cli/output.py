"""Utilities specific to formatting the output of the CLI.
"""

from .lang import get_raw as _raw

import shutil
import time
import sys
import re

from typing import List, Tuple, Union, Optional


class OutputTable:
    """Base class for formatting tables.
    """

    def __init__(self) -> None:
        self.rows: List[Union[None, Tuple[str, ...]]] = []
        self.columns_length: List[int] = []

    def add(self, *cells):
        """Add a row to the table.
        """

        cells_str = tuple(map(str, cells))
        self.rows.append(cells_str)

        columns_count = len(self.columns_length)
        
        # Limit to the current columns count.
        for i, cell in enumerate(cells_str[:columns_count]):
            cell_len = len(cell)
            if self.columns_length[i] < cell_len:
                self.columns_length[i] = cell_len

        # Adjust if the inserted cells have more columns that current state. 
        if columns_count < len(cells_str):
            self.columns_length.extend(map(len, cells_str[columns_count:]))
    
    def separator(self) -> None:
        """Add a separator to the table.
        """
        self.rows.append(None)
    
    def print(self) -> None:
        """Print the table to the output.
        """
        raise NotImplementedError


class Output:
    """This class is used to abstract the output of the CLI. This particular class is
    abstract and the implementation differs depending on the desired output format.
    """

    def table(self) -> OutputTable:
        """Create a table builder that you can use to add rows and separator and them
        print a table, adapted to the implementation.
        """
        raise NotImplementedError
    
    def task(self, state: Optional[str], key: Optional[str], **kwargs) -> None:
        """Update the current task (or create it if not the case).
        """
        raise NotImplementedError
    
    def finish(self) -> None:
        """Finish any active task.
        """
        raise NotImplementedError

    def print(self, text: str) -> None:
        """Raw print of the given text, this is commonly used to forward game's standard
        output/error streams. The implementor may apply some style before printing text.
        This function doesn't add any new line
        """
        raise NotImplementedError

    def prompt(self, password: bool = False) -> Optional[str]:
        """Prompt for a line to come on standard input.
        """
        raise NotImplementedError


class HumanOutput(Output):

    state_colors = {
        "OK": "\033[92m",
        "FAILED": "\033[31m",
        "WARN": "\033[33m",
        "INFO": "\033[34m",
        "HALT": "\033[33m",
    }

    print_colors = [
        ("ERROR", "\033[31m"),
        ("WARN", "\033[33m"),
        ("SEVERE", "\033[31m"),
        ("FATAL", "\033[31m"),
    ]

    def __init__(self, color: bool) -> None:
        super().__init__()
        self.term_width = 0
        self.term_width_update_time = 0
        self.last_len = None
        self.color = color

    def get_term_width(self) -> int:
        """Internal method used to get terminal width with a cache interval of 1 second.
        """
        now = time.monotonic()
        if now - self.term_width_update_time > 1:
            self.term_width_update_time = now
            self.term_width = shutil.get_terminal_size().columns
        return self.term_width

    def table(self) -> OutputTable:
        return HumanTable(self)
    
    def task(self, state: Optional[str], key: Optional[str], **kwargs) -> None:

        # Don't display updates on small terminals (9 for the state, 11 for msg).
        term_width = self.get_term_width()
        if term_width < 20:
            return
        
        # Get header for the given state (with optional color).
        if state is None:
            state_msg = "\r         "
        else:
            color = self.state_colors.get(state) if self.color else None
            if color is not None:
                state_msg = f"\r[{color}{state:^6s}\033[0m] "
            else:
                state_msg = f"\r[{state:^6s}] "

        print(state_msg, end="", flush=False)

        if key is None:
            self.last_len = 0
            sys.stdout.flush()
            return

        msg = _raw(key, kwargs)
        if len(msg) + 9 > term_width:
            msg = f"{msg[:term_width - 9 - 3]}..."
        
        msg_len = len(msg)

        print(msg, end="", flush=False)

        if self.last_len is not None and self.last_len > msg_len:
            missing_len = self.last_len - msg_len
            print(" " * missing_len, end="", flush=False)
        
        sys.stdout.flush()

        self.last_len = msg_len

    def finish(self) -> None:
        if self.last_len is not None:
            print()
            self.last_len = None
    
    def print(self, text: str) -> None:
        
        if self.color:

            chosen_color = None
            for token, code in self.print_colors:
                if token in text:
                    chosen_color = code
                    break
            
            if chosen_color is not None:
                print(chosen_color, text, "\033[0m", sep="", end="")
                return
        
        print(text, end="")
    
    def prompt(self, password: bool = False) -> Optional[str]:
        try:
            if password:
                import getpass
                return getpass.getpass("")
            else:
                return input("")
        except KeyboardInterrupt:
            return None

class HumanTable(OutputTable):
    
    def __init__(self, out: HumanOutput) -> None:
        super().__init__()
        self.out = out

    def print(self) -> None:

        columns_length = self.columns_length.copy()
        columns_count = len(columns_length)

        total_length = 1 + sum(x + 3 for x in columns_length)
        max_length = self.out.get_term_width() - 1
        if total_length > max_length:
            
            overflow_length = total_length - max_length
            total_columns_length = sum(columns_length)

            for i in range(columns_count):
                column_ratio = columns_length[i] / max_length
                cell_overflow_length = int(columns_length[i] / total_columns_length * column_ratio * overflow_length)
                overflow_length -= cell_overflow_length
                columns_length[i] -= cell_overflow_length
                if i == columns_count - 1:
                    columns_length[i] -= overflow_length
        
        format_string = "│ {} │".format(" │ ".join((f"{{:{length}s}}" for length in columns_length)))
        columns_lines = ["─" * length for length in columns_length]

        print("┌─{}─┐".format("─┬─".join(columns_lines)), flush=False)

        format_columns = [""] * columns_count

        for row in self.rows:

            if row is None:
                print("├─{}─┤".format("─┼─".join(columns_lines)), flush=False)
                continue

            wrapped_row = list(row)
            wrapped = True

            while wrapped:
                wrapped = False
                for col_index, col in enumerate(wrapped_row):
                    col_len = columns_length[col_index]
                    col_real = col[:col_len]
                    format_columns[col_index] = col_real
                    # If wrapped, take the rest and save it for next iteration.
                    if col != col_real:
                        wrapped_row[col_index] = f" {col[col_len:]}"
                        wrapped = True
                    else:
                        wrapped_row[col_index] = ""
                
                print(format_string.format(*format_columns), flush=False)
        
        print("└─{}─┘".format("─┴─".join(columns_lines)))


class MachineOutput(Output):

    escape_re = re.compile("[\\n\\r,]")

    @classmethod
    def print_escape(cls, s: str) -> str:
        return re.sub(cls.escape_re, lambda match: "\\" + {10: "n", 13: "r"}.get(ord(match.group()), match.group()), s)

    def print_function(self, name: str, *args: str, **kwargs) -> None:
        """Print a machine-readable line for a function with some parameters.
        """
        print(name, ":", ",".join((self.print_escape(arg) for arg in [
            *args,
            *(f"{k}={v}" for k, v in kwargs.items())  # Note, k should not contain "="
        ])), sep="")

    def table(self) -> OutputTable:
        return MachineTable(self)
    
    def task(self, state: Optional[str], key: Optional[str], **kwargs) -> None:
        self.print_function("task", str(state), str(key), **kwargs)
    
    def finish(self) -> None:
        pass

    def print(self, text: str) -> None:
        self.print_function("print", text)
    
    def prompt(self, password: bool = False) -> Optional[str]:
        self.print_function("prompt", password=str(int(password)))
        try:
            return input("")
        except KeyboardInterrupt:
            return None

class MachineTable(OutputTable):

    def __init__(self, out: MachineOutput) -> None:
        super().__init__()
        self.out = out
    
    def print(self) -> None:
        self.out.print_function("table", str(len(self.rows)))
        for row in self.rows:
            if row is None:
                self.out.print_function("sep")
            else:
                self.out.print_function("row", *row)
