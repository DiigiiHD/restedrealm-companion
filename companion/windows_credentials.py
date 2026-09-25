"""Store the upload-only token in Windows Credential Manager, outside the queue."""

import ctypes
from ctypes import wintypes
import json
import os


TARGET = "RestedRealmCollector/restedrealm.com"
CRED_TYPE_GENERIC = 1
CRED_PERSIST_LOCAL_MACHINE = 2
ERROR_NOT_FOUND = 1168


class FILETIME(ctypes.Structure):
    _fields_ = [("dwLowDateTime", wintypes.DWORD), ("dwHighDateTime", wintypes.DWORD)]


class CREDENTIALW(ctypes.Structure):
    _fields_ = [
        ("Flags", wintypes.DWORD), ("Type", wintypes.DWORD),
        ("TargetName", wintypes.LPWSTR), ("Comment", wintypes.LPWSTR),
        ("LastWritten", FILETIME), ("CredentialBlobSize", wintypes.DWORD),
        ("CredentialBlob", ctypes.POINTER(ctypes.c_ubyte)),
        ("Persist", wintypes.DWORD), ("AttributeCount", wintypes.DWORD),
        ("Attributes", ctypes.c_void_p), ("TargetAlias", wintypes.LPWSTR),
        ("UserName", wintypes.LPWSTR),
    ]


def api():
    if os.name != "nt":
        raise OSError("Windows Credential Manager is required")
    library = ctypes.WinDLL("Advapi32", use_last_error=True)
    library.CredWriteW.argtypes = [ctypes.POINTER(CREDENTIALW), wintypes.DWORD]
    library.CredWriteW.restype = wintypes.BOOL
    library.CredReadW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                  ctypes.POINTER(ctypes.POINTER(CREDENTIALW))]
    library.CredReadW.restype = wintypes.BOOL
    library.CredDeleteW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD]
    library.CredDeleteW.restype = wintypes.BOOL
    library.CredFree.argtypes = [ctypes.c_void_p]
    library.CredFree.restype = None
    return library


def save(device_id: str, token: str):
    value = json.dumps({"deviceId": device_id, "token": token}, separators=(",", ":")).encode("utf-8")
    blob = (ctypes.c_ubyte * len(value)).from_buffer_copy(value)
    credential = CREDENTIALW()
    credential.Type = CRED_TYPE_GENERIC
    credential.TargetName = TARGET
    credential.Comment = "RestedRealm Collector upload-only credential"
    credential.CredentialBlobSize = len(value)
    credential.CredentialBlob = ctypes.cast(blob, ctypes.POINTER(ctypes.c_ubyte))
    credential.Persist = CRED_PERSIST_LOCAL_MACHINE
    credential.UserName = "RestedRealm Collector"
    library = api()
    if not library.CredWriteW(ctypes.byref(credential), 0):
        raise ctypes.WinError(ctypes.get_last_error())


def load():
    library = api()
    pointer = ctypes.POINTER(CREDENTIALW)()
    if not library.CredReadW(TARGET, CRED_TYPE_GENERIC, 0, ctypes.byref(pointer)):
        if ctypes.get_last_error() == ERROR_NOT_FOUND:
            return None
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        value = ctypes.string_at(pointer.contents.CredentialBlob,
                                 pointer.contents.CredentialBlobSize)
        credential = json.loads(value.decode("utf-8"))
        if not isinstance(credential.get("token"), str) or not isinstance(credential.get("deviceId"), str):
            raise ValueError("Stored Collector credential is invalid")
        return credential
    finally:
        library.CredFree(ctypes.cast(pointer, ctypes.c_void_p))


def forget():
    library = api()
    if not library.CredDeleteW(TARGET, CRED_TYPE_GENERIC, 0):
        if ctypes.get_last_error() != ERROR_NOT_FOUND:
            raise ctypes.WinError(ctypes.get_last_error())
