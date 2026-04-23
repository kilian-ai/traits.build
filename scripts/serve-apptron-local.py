#!/usr/bin/env python3
"""Local HTTP server for apptron barebones testing.
Adds COOP/COEP headers required for SharedArrayBuffer (needed by wanix/v86).
"""
import http.server, socketserver, sys, os

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9876
DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'traits/www/static/apptron')

class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=DIR, **kw)
    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Resource-Policy', 'cross-origin')
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()

with socketserver.TCPServer(('127.0.0.1', PORT), H) as httpd:
    print(f'serving {DIR} on http://127.0.0.1:{PORT}/barebones.html')
    httpd.serve_forever()
