"""Utilities specific to formatting the output of the CLI.
"""

from . import lang

import shutil
import time
import json

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


class OutputTask:
    
    def update(self, state: Optional[str], key: Optional[str], **kwargs) -> None:
        raise NotImplementedError

    def finish(self) -> None:
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
    
    def task(self) -> OutputTask:
        raise NotImplementedError


class HumanOutput(Output):

    def __init__(self) -> None:
        super().__init__()
        self.term_width = 0
        self.term_width_update_time = 0

    def table(self) -> OutputTable:
        return HumanTable(self)
    
    def task(self) -> OutputTask:
        return HumanTask(self)

    def get_term_width(self) -> int:
        """Internal method used to get terminal width with a cache interval of 1 second.
        """
        now = time.monotonic()
        if now - self.term_width_update_time > 1:
            self.term_width_update_time = now
            self.term_width = shutil.get_terminal_size().columns
        return self.term_width

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
            total_cell_length = sum(columns_length)
            for i in range(columns_count):
                cell_overflow_length = int(columns_length[i] / total_cell_length * overflow_length)
                overflow_length -= cell_overflow_length
                columns_length[i] -= cell_overflow_length
                if i == columns_count - 1:
                    columns_length[i] -= overflow_length
        
        format_string = "│ {} │".format(" │ ".join((f"{{:{length}s}}" for length in columns_length)))
        columns_lines = ["─" * length for length in columns_length]

        print("┌─{}─┐".format("─┬─".join(columns_lines)))

        format_columns = [""] * columns_count

        for row in self.rows:

            if row is None:
                print("├─{}─┤".format("─┼─".join(columns_lines)))
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
                        wrapped_row[col_index] = col[col_len:]
                        wrapped = True
                
                print(format_string.format(*format_columns))
        
        print("└─{}─┘".format("─┴─".join(columns_lines)))

class HumanTask(OutputTask):

    def __init__(self, out) -> None:
        super().__init__()
        self.out = out
        self.last_len = None

    def update(self, state: Optional[str], key: Optional[str], **kwargs) -> None:

        state_msg = "\r         " if state is None else "\r[{:^6s}] ".format(state)
        print(state_msg, end="")

        if key is None:
            return

        msg = lang.get_raw(key, kwargs)
        msg_len = len(msg)

        print(msg, end="")

        if self.last_len is not None and self.last_len > msg_len:
            missing_len = self.last_len - msg_len
            print(" " * missing_len, end="")

        self.last_len = msg_len

    def finish(self) -> None:
        if self.last_len is not None:
            print()


class JsonOutput(Output):
    
    def table(self) -> OutputTable:
        return JsonTable()

    def task(self) -> OutputTask:
        return JsonTask()

class JsonTable(OutputTable):

    def print(self) -> None:
        print(json.dumps(self.rows, indent=2))

class JsonTask(OutputTask):

    def update(self, state: Optional[str], key: Optional[str], **kwargs) -> None:
        print(json.dumps({"state": state, "key": key, "args": kwargs}))

    def finish(self) -> None:
        pass
