"""Local capture server: serves dist/viewer and answers the public ladder route
with the documented empty shape, so the directory shows the honest empty state.
It never returns ratings. Usage: python3 serve.py PORT ROOT"""
import http.server, json, sys, functools, urllib.parse

class H(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        u = urllib.parse.urlparse(self.path)
        if u.path == "/v1/leaderboard":
            mode = urllib.parse.parse_qs(u.query).get("mode", ["1v1"])[0]
            body = json.dumps({"season": "season-1", "mode": mode, "entries": []}).encode()
            self.send_response(200); self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
            return
        super().do_GET()
    def log_message(self, *a): pass

http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), functools.partial(H, directory=sys.argv[2])).serve_forever()
