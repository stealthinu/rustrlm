import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
import unittest


class _Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):  # noqa: A003
        return

    def do_GET(self):
        if self.path == "/v1/health":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok", "name": "rustrlm"}).encode())
            return
        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        if self.path == "/v1/retrieve":
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length)
            req = json.loads(body.decode())
            q = req.get("query", "")
            docs = req.get("documents", [])
            results = []
            for d in docs:
                if q.lower() in d.get("text", "").lower():
                    results.append({
                        "doc_id": d.get("id"),
                        "score": 1.0,
                        "text": d.get("text"),
                        "metadata": d.get("metadata"),
                        "spans": [{"start": 0, "end": min(10, len(d.get("text","")))}],
                    })
            resp = {"trace_id": "test", "results": results, "warnings": []}
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(resp).encode())
            return
        self.send_response(404)
        self.end_headers()


class RustRLMClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.httpd = HTTPServer(("127.0.0.1", 0), _Handler)
        cls.addr = cls.httpd.server_address
        cls.thread = threading.Thread(target=cls.httpd.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.httpd.shutdown()
        cls.thread.join(timeout=1)

    def test_health(self):
        from rustrlm_client import RustRLMClient

        client = RustRLMClient(base_url=f"http://{self.addr[0]}:{self.addr[1]}")
        body = client.health()
        self.assertEqual(body["status"], "ok")
        self.assertEqual(body["name"], "rustrlm")

    def test_retrieve(self):
        from rustrlm_client import RustRLMClient

        client = RustRLMClient(base_url=f"http://{self.addr[0]}:{self.addr[1]}")
        res = client.retrieve(
            "brown",
            [
                {"id": "a", "text": "alpha"},
                {"id": "b", "text": "quick brown fox"},
            ],
        )
        self.assertEqual(len(res["results"]), 1)
        self.assertEqual(res["results"][0]["doc_id"], "b")

    def test_langchain_retriever(self):
        try:
            from rustrlm_client import RustRLMClient
            from rustrlm_client.integrations.langchain import RustRLMRetriever
        except Exception as exc:
            self.skipTest(f"langchain-core not available: {exc}")

        client = RustRLMClient(base_url=f"http://{self.addr[0]}:{self.addr[1]}")
        retriever = RustRLMRetriever(
            client,
            options={
                "documents": [
                    {"id": "a", "text": "alpha"},
                    {"id": "b", "text": "quick brown fox"},
                ]
            },
        )
        if hasattr(retriever, "get_relevant_documents"):
            docs = retriever.get_relevant_documents("brown")
        else:
            docs = retriever.invoke("brown")
        self.assertEqual(len(docs), 1)
        self.assertEqual(docs[0].metadata.get("doc_id"), "b")


if __name__ == "__main__":
    unittest.main()
