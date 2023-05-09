"""Base utilities for task-based installer.
"""

from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from typing import List, Type


class TaskError(Exception):
    """A generic task error with a code and 
    """


class Task:
    """Represent a task that can be run in the installer.
    """

    def setup(self, state: dict) -> None:
        """Initialize the installer's state data, this is called when
        the task is added to an installer. The order of these calls
        should not be important, and conflicting states are not 
        handled by installers.

        :param state: The global installer's state data.
        """
    
    def execute(self, state: dict) -> None:
        """Execute the task with a given piece of data.

        :param state: The installer's state data, this can be used to 
        transmit data to future tasks that requires parameters. This 
        data can also be used to alt the installer and resume it.
        :raises NotImplementedError: Raised if this function is not
        implemented by subclasses.
        """
        raise NotImplementedError


class Installer:
    """A task-based installer.
    """

    def __init__(self) -> None:
        self.tasks: List[Task] = []
        self.state: dict = {}

    def add(self, task: "Type[Task]") -> None:
        """Add a task to be executed by this installer.

        :param task: The task to add to this installer.
        """
        self.tasks.append(task)
        task.setup(self.state)

    def reset(self) -> None:
        """Reset the internal state and re-init all tasks.
        """
        self.state = {}
        for task in self.tasks:
            task.setup(self.state)

    def install(self) -> None:
        """Sequentially execute the tasks of this installer.
        """
        for task in self.tasks:
            task.execute(self.state)


class Watcher:
    """Base class for a watcher of the install process.
    """

    def on_task(self, name: str) -> None:
        """Called when a task is being executed.
        """
