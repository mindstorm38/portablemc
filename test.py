from portablemc.optifine import OptifineVersion, get_versions_list, get_compatible_versions, get_offline_versions
from portablemc.standard import VersionHandle, Watcher, SimpleWatcher
from portablemc.http import http_request
from pathlib import Path
class prwatch(Watcher):
    def handle(self, event):
        print(event)
        for attr in dir(event):
            pass
            #print(attr)
if __name__ == "__main__":
    #get_versions_list()
    #get_compatible_versions()
    print(get_compatible_versions())
    get_offline_versions(Path("/home/pi-dev500/.minecraft/versions"))
    v = OptifineVersion("1.7.10")
    env=v.install(watcher=prwatch())
    env.run()
