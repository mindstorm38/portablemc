

from .task import Installer
from .task_dl import DownloadTask


def get_installer() -> Installer:

    installer = Installer()
    installer.add(DownloadTask())

    return installer
