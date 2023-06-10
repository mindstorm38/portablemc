"""Utilities specific to formatting the output of the CLI.
"""

import shutil
import time

from typing import Iterable, Any, List, Tuple, Union


class Table:
    """
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

    def table(self) -> Table:
        """Create a table builder that you can use to add rows and separator and them
        print a table, adapted to the implementation.
        """
        raise NotImplementedError
    
    def task(self):
        pass


class HumanOutput(Output):

    def table(self) -> Table:
        return HumanTable()

class HumanTable(Table):
    
    def __init__(self) -> None:
        super().__init__()
        self.term_width = 0
        self.term_width_update_time = 0

    def print(self) -> None:

        columns_length = self.columns_length.copy()
        columns_count = len(columns_length)

        total_length = 1 + sum(x + 3 for x in columns_length)
        max_length = self.get_term_width() - 1
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

        for row in self.rows:

            if row is None:
                print("├─{}─┤".format("─┼─".join(columns_lines)))
                continue

            wrapped = True
            while wrapped:

                cols = []
                wrapped = False

                for col_index, col in enumerate(row):
                    col_len = columns_length[col_index]
                    col_real = col[:col_len]
                    cols.append(col)
                    if col != col_real:
                        row[col_index] = col[col_len:]
                        wrapped = True
                
                print(format_string.format(*cols))
        
        print("└─{}─┘".format("─┴─".join(columns_lines)))

    def get_term_width(self) -> int:
        """Internal method used to get terminal width with a cache interval of 1 second.
        """
        now = time.monotonic()
        if now - self.term_width_update_time > 1:
            self.term_width_update_time = now
            self.term_width = shutil.get_terminal_size().columns
        return self.term_width


class JsonOutput(Output):
    pass

class JsonTable(Table):

    def print(self) -> None:
        import json
        print(json.dumps(self.rows, indent=2))
