"""Mojang and Microsoft authentication utilities.
"""

from urllib import parse as url_parse
from uuid import UUID, uuid4, uuid5
from pathlib import Path
import platform
import base64
import json

from .http import HttpError, http_request

from typing import Optional, Dict, Type, Tuple


class AuthSession:
    """An abstract class for defining authentication sessions. These sessions are then
    provided as an argument for starting the game. They provide all information such as
    access player's token, username or UUID.

    Different class variables must be defined by subclasses: `db_type` which is used when
    saving the session to the database, `user_type` is an information sent through command
    line to the game and `fields` which defines all class's object's fields that should
    be saved and restored from the database.

    As stated above, the session class is closely related to `AuthDatabase` in which they
    are saved and later restored.
    """

    db_type: str
    user_type: str
    fields = "access_token", "username", "uuid", "client_id"

    @classmethod
    def fix_data(cls, data: dict) -> None:
        """This optional function may be used by subclass to provide data migration from
        older database formats. The input data can be modified as needed to fit currently
        required fields (specified in `fields` class variable).

        :param data: The data that can be modified if relevant.
        """

    def __init__(self):
        self.access_token = ""
        self.username = ""
        self.uuid = ""
        self.client_id = ""

    def format_token_argument(self, legacy: bool) -> str:
        """Format the token for the game's command line. Modern versions uses the format
        `token:{access_token}:{uuid}` and legacy versions uses `{access_token}`.

        :param legacy: True to enable legacy formatting, used by older versions.
        :return: The formatted token.
        """
        return f"token:{self.access_token}:{self.uuid}" if legacy else self.access_token

    def get_xuid(self) -> str:
        """Getter specific to Microsoft, but common to auth sessions because it's used for
        Minecraft's command line arguments.
        """
        return ""

    def validate(self) -> bool:
        """Validate that the current session is still actually authenticating the player.

        :return: True if the session is still valid, when returning false the `refresh`
        method may be called to update the session.
        """
        return True

    def refresh(self) -> None:
        """Try refreshing the session to make it valid again.
        """

    def invalidate(self) -> None:
        """Invalid the session so that `validate` will now return false.
        """


class OfflineAuthSession(AuthSession):
    """Offline session, this is quite contradictory but it's actually useful to simplify
    the start logic. It provides optional static username and UUID and random when kept
    unspecified.
    """

    db_type = "offline"
    user_type = ""

    def __init__(self, username: Optional[str], uuid: Optional[str]):
        super().__init__()
        if uuid is not None and len(uuid) == 32:
            # If the UUID is already valid.
            self.uuid = uuid
            self.username = uuid[:8] if username is None else username[:16]
        else:
            namespace_hash = UUID("8df5a464-38de-11ec-aa66-3fd636ee2ed7")
            if username is None:
                self.uuid = uuid5(namespace_hash, platform.node()).hex
                self.username = self.uuid[:8]
            else:
                self.username = username[:16]
                self.uuid = uuid5(namespace_hash, self.username).hex

    def format_token_argument(self, legacy: bool) -> str:
        return ""


class YggdrasilAuthSession(AuthSession):
    """Yggdrasil authentication (deprecated). This authentication is now deprecated but
    was also known as "Mojang authentication".
    """

    db_type = "yggdrasil"
    user_type = "mojang"

    @classmethod
    def fix_data(cls, data: dict):
        if "client_token" in data:
            data["client_id"] = data.pop("client_token")

    def validate(self) -> bool:
        return self.request("validate", {
            "accessToken": self.access_token,
            "clientToken": self.client_id
        }, False)[0] == 204

    def refresh(self):
        _, res = self.request("refresh", {
            "accessToken": self.access_token,
            "clientToken": self.client_id
        })
        self.access_token = res["accessToken"]
        self.username = res["selectedProfile"]["name"]  # Refresh username if renamed (does it works? to check.).

    def invalidate(self):
        self.request("invalidate", {
            "accessToken": self.access_token,
            "clientToken": self.client_id
        }, False)

    @classmethod
    def authenticate(cls, client_id: str, email: str, password: str) -> 'YggdrasilAuthSession':
        _, res = cls.request("authenticate", {
            "agent": {
                "name": "Minecraft",
                "version": 1
            },
            "username": email,
            "password": password,
            "clientToken": client_id
        })
        sess = cls()
        sess.access_token = res["accessToken"]
        sess.username = res["selectedProfile"]["name"]
        sess.uuid = res["selectedProfile"]["id"]
        sess.client_id = res["clientToken"]
        return sess

    @classmethod
    def request(cls, req: str, payload: dict, raise_error: bool = True) -> Tuple[int, dict]:
        try:
            res = http_request("POST", f"https://authserver.mojang.com/{req}", 
                data=json.dumps(payload).encode("ascii"),
                accept="application/json",
                content_type="application/json")
            return res.status, res.json()
        except HttpError as error:
            if raise_error:
                raise AuthError(error.res.json()["errorMessage"])
            else:
                return error.res.status, error.res.json()


class MicrosoftAuthSession(AuthSession):
    """Microsoft authentication for Minecraft. It involves multiples endpoint from
    Mojang, MSA and XBox Live.
    """

    db_type = "microsoft"
    user_type = "msa"
    fields = "access_token", "username", "uuid", "client_id", "refresh_token", "app_id", "redirect_uri", "xuid"

    @classmethod
    def fix_data(cls, data: dict):
        if "app_id" not in data and "client_id" in data:
            data["app_id"] = data.pop("client_id")
        if "client_id" not in data or not len(data["client_id"]):
            data["client_id"] = str(uuid4())
        if "xuid" not in data:
            data["xuid"] = cls.decode_jwt_payload(data["access_token"])["xuid"]

    def __init__(self):
        super().__init__()
        self.refresh_token = ""
        self.app_id = ""
        self.redirect_uri = ""
        self.xuid = ""
        self._new_username: Optional[str] = None

    def get_xuid(self) -> str:
        return self.xuid

    def validate(self) -> bool:
        self._new_username = None
        try:
            res = self.mc_request_profile(self.access_token)
            username = res["name"]
            if self.username != username:
                self._new_username = username
                return False
            return True
        except HttpError:
            return False

    def refresh(self):
        if self._new_username is not None:
            self.username = self._new_username
            self._new_username = None
        else:
            res = self.authenticate_base({
                "client_id": self.app_id,
                "redirect_uri": self.redirect_uri,
                "refresh_token": self.refresh_token,
                "grant_type": "refresh_token",
                "scope": "xboxlive.signin"
            })
            self.access_token = res["access_token"]
            self.username = res["username"]
            self.uuid = res["uuid"]

    @staticmethod
    def get_authentication_url(app_id: str, redirect_uri: str, email: str, nonce: str):
        return "https://login.live.com/oauth20_authorize.srf?{}".format(url_parse.urlencode({
            "client_id": app_id,
            "redirect_uri": redirect_uri,
            "response_type": "code id_token",
            "scope": "xboxlive.signin offline_access openid email",
            "login_hint": email,
            "nonce": nonce,
            "response_mode": "form_post"
        }))

    @staticmethod
    def get_logout_url(app_id: str, redirect_uri: str):
        return "https://login.live.com/oauth20_logout.srf?{}".format(url_parse.urlencode({
            "client_id": app_id,
            "redirect_uri": redirect_uri
        }))

    @classmethod
    def check_token_id(cls, token_id: str, email: str, nonce: str) -> bool:
        id_token_payload = cls.decode_jwt_payload(token_id)
        return id_token_payload["nonce"] == nonce and id_token_payload["email"].casefold() == email.casefold()

    @classmethod
    def authenticate(cls, client_id: str, app_id: str, code: str, redirect_uri: str) -> 'MicrosoftAuthSession':
        res = cls.authenticate_base({
            "client_id": app_id,
            "redirect_uri": redirect_uri,
            "code": code,
            "grant_type": "authorization_code",
            "scope": "xboxlive.signin"
        })
        sess = cls()
        sess.access_token = res["access_token"]
        sess.username = res["username"]
        sess.uuid = res["uuid"]
        sess.client_id = client_id
        sess.refresh_token = res["refresh_token"]
        sess.app_id = app_id
        sess.redirect_uri = redirect_uri
        sess.xuid = cls.decode_jwt_payload(res["access_token"])["xuid"]
        return sess

    @classmethod
    def authenticate_base(cls, request_token_payload: dict) -> dict:

        # Microsoft OAuth
        try:
            res = cls.ms_request("https://login.live.com/oauth20_token.srf", request_token_payload, payload_url_encoded=True)
        except HttpError:
            raise OutdatedTokenError()

        ms_refresh_token = res.get("refresh_token")

        # Xbox Live Token
        res = cls.ms_request("https://user.auth.xboxlive.com/user/authenticate", {
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": "d={}".format(res["access_token"])
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        })

        xbl_token = res["Token"]
        xbl_user_hash = res["DisplayClaims"]["xui"][0]["uhs"]

        # Xbox Live XSTS Token
        res = cls.ms_request("https://xsts.auth.xboxlive.com/xsts/authorize", {
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        })
        xsts_token = res["Token"]

        if xbl_user_hash != res["DisplayClaims"]["xui"][0]["uhs"]:
            raise AuthError("inconsistent user hash")

        # MC Services Auth
        res = cls.ms_request("https://api.minecraftservices.com/authentication/login_with_xbox", {
            "identityToken": f"XBL3.0 x={xbl_user_hash};{xsts_token}"
        })
        mc_access_token = res["access_token"]

        # MC Services Profile
        try:
            res = cls.mc_request_profile(mc_access_token)
        except HttpError as error:
            if error.res.status == 404:
                raise DoesNotOwnMinecraftError()
            elif error.res.status == 401:
                raise OutdatedTokenError()
            else:
                res = error.res.json()
                raise AuthError(res.get("errorMessage", res.get("error", "Unknown error")))

        return {
            "refresh_token": ms_refresh_token,
            "access_token": mc_access_token,
            "username": res["name"],
            "uuid": res["id"]
        }

    @classmethod
    def ms_request(cls, url: str, payload: dict, *, payload_url_encoded: bool = False) -> dict:
        data = (url_parse.urlencode(payload) if payload_url_encoded else json.dumps(payload)).encode("ascii")
        content_type = "application/x-www-form-urlencoded" if payload_url_encoded else "application/json"
        return http_request("POST", url, data=data, content_type=content_type).json()

    @classmethod
    def mc_request_profile(cls, bearer: str) -> dict:
        url = "https://api.minecraftservices.com/minecraft/profile"
        return http_request("GET", url, headers={"Authorization": f"Bearer {bearer}"}).json()

    @classmethod
    def base64url_decode(cls, s: str) -> bytes:
        rem = len(s) % 4
        if rem > 0:
            s += "=" * (4 - rem)
        return base64.urlsafe_b64decode(s)

    @classmethod
    def decode_jwt_payload(cls, jwt: str) -> dict:
        return json.loads(cls.base64url_decode(jwt.split(".")[1]))


class AuthDatabase:
    """The authentication database used to keep sessions stored. It also keeps a clien
    id which is common to all sessions to identify the client.
    """

    types = {
        YggdrasilAuthSession.db_type: YggdrasilAuthSession,
        MicrosoftAuthSession.db_type: MicrosoftAuthSession
    }

    def __init__(self, file: Path):
        self.file = file
        self.sessions: Dict[str, Dict[str, AuthSession]] = {}
        self.client_id: Optional[str] = None

    def load(self):

        self.sessions.clear()

        try:
            with self.file.open("rt") as fp:
                data = json.load(fp)
                self.client_id = data.get("client_id")
                for typ, sess_type in self.types.items():
                    typ_data = data.get(typ)
                    if typ_data is not None:
                        sessions = self.sessions[typ] = {}
                        sessions_data = typ_data["sessions"]
                        for email, sess_data in sessions_data.items():
                            # Use class method fix_data to migrate data from older versions of the auth database.
                            sess_type.fix_data(sess_data)
                            sess = sess_type()
                            for field in sess_type.fields:
                                setattr(sess, field, sess_data.get(field, ""))
                            sessions[email.casefold()] = sess
        except (OSError, KeyError, TypeError, json.JSONDecodeError):
            pass

    def save(self) -> None:

        self.file.parent.mkdir(parents=True, exist_ok=True)

        with self.file.open("wt") as fp:
            data = {}
            if self.client_id is not None:
                data["client_id"] = self.client_id
            for typ, sessions in self.sessions.items():
                if typ not in self.types:
                    continue
                sess_type = self.types[typ]
                sessions_data = {}
                data[typ] = {"sessions": sessions_data}
                for email, sess in sessions.items():
                    sess_data = sessions_data[email] = {}
                    for field in sess_type.fields:
                        sess_data[field] = getattr(sess, field)
            json.dump(data, fp, indent=2)

    def get(self, email: str, sess_type: Type[AuthSession]) -> Optional[AuthSession]:
        """Try to get a session from an email and session type.
        """
        sessions = self.sessions.get(sess_type.db_type)
        return None if sessions is None else sessions.get(email.casefold())

    def put(self, email: str, sess: AuthSession):
        """Push the given authentication session to the database, updating any previous
        session with the same email for the type of session.
        """
        sessions = self.sessions.get(sess.db_type)
        if sessions is None:
            if sess.db_type not in self.types:
                raise ValueError(f"given session type '{sess.db_type}' is not supported")
            sessions = self.sessions[sess.db_type] = {}
        sessions[email.casefold()] = sess

    def remove(self, email: str, sess_type: Type[AuthSession]) -> Optional[AuthSession]:
        """Same arguments as `get` method but remove the session and return it.
        """
        email = email.casefold()
        sessions = self.sessions.get(sess_type.db_type)
        if sessions is not None:
            session = sessions.get(email)
            if session is not None:
                del sessions[email]
                return session

    def get_client_id(self) -> str:
        if self.client_id is None or len(self.client_id) != 36:
            self.client_id = str(uuid4())
        return self.client_id


class AuthError(Exception):
    pass

class DoesNotOwnMinecraftError(AuthError):
    pass

class OutdatedTokenError(AuthError):
    pass
