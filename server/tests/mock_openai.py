import json
import http.server


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps({"data": [{"id": "mock-1"}, {"id": "mock-2"}]}).encode())

    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0))
        body = json.loads(self.rfile.read(n) or b'{}')
        last = body['messages'][-1]['content']
        text = last if isinstance(last, str) else json.dumps(last, ensure_ascii=False)
        if '判重助手' in text:
            out = '{"same":false}'
        elif '总结专家' in text:
            out = '{"answer":"传动角"}'
        else:
            out = '分析：从动件运动规律由传动角决定。\n{"answer":"B"}'
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for ch in ['思考中', '...']:
            self.wfile.write(('data: ' + json.dumps({"choices": [{"delta": {"reasoning_content": ch}}]}) + '\n\n').encode())
        for i in range(0, len(out), 7):
            self.wfile.write(('data: ' + json.dumps({"choices": [{"delta": {"content": out[i:i + 7]}}]}, ensure_ascii=False) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


http.server.HTTPServer(('127.0.0.1', 18081), Handler).serve_forever()
