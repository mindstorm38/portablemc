"""Utilities specific to formatting the output of the CLI.
"""

from typing import Iterable, Any, List, Tuple, Union


class Table:
    """
    """

    def __init__(self) -> None:
        self.rows: List[Union[None, Tuple[str, ...]]] = []
        self.columns_length: List[int] = []

    def add(self, cells: Iterable[Any]):
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
    pass
