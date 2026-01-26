import json
import urllib.request
import urllib.error
from typing import Any, Dict, List, Optional


class RustRLMError(RuntimeError):
    pass


class RustRLMClient:
    def __init__(self, base_url: str = "http://127.0.0.1:8080", timeout: int = 30) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def health(self) -> Dict[str, Any]:
        return self._get("/v1/health")

    def retrieve(
        self,
        query: str,
        documents: List[Dict[str, Any]],
        options: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        payload = {"query": query, "documents": documents}
        if options is not None:
            payload["options"] = options
        return self._post("/v1/retrieve", payload)

    def _get(self, path: str) -> Dict[str, Any]:
        url = self.base_url + path
        req = urllib.request.Request(url, method="GET")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return json.loads(resp.read().decode())
        except urllib.error.HTTPError as e:
            raise RustRLMError(f"http {e.code}: {e.read().decode(errors='ignore')}") from e
        except Exception as e:
            raise RustRLMError(str(e)) from e

    def _post(self, path: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        url = self.base_url + path
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(url, data=data, method="POST")
        req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return json.loads(resp.read().decode())
        except urllib.error.HTTPError as e:
            raise RustRLMError(f"http {e.code}: {e.read().decode(errors='ignore')}") from e
        except Exception as e:
            raise RustRLMError(str(e)) from e
