#!/usr/bin/env python3
import http.server
import socketserver
import os
import sys

PORT = 5173
DIRECTORY = os.path.dirname(os.path.abspath(__file__))

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)

def main():
    port = PORT
    for attempt in range(5):
        try:
            with socketserver.TCPServer(("", port), Handler) as httpd:
                import socket
                try:
                    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    s.connect(("8.8.8.8", 80))
                    local_ip = s.getsockname()[0]
                    s.close()
                except Exception:
                    local_ip = socket.gethostbyname(socket.gethostname())

                print(f"Reliable UDP Protocol Inspector running at:")
                print(f"  • Local:   http://localhost:{port}")
                print(f"  • Network: http://{local_ip}:{port}")
                sys.stdout.flush()
                httpd.serve_forever()
        except OSError:
            port += 1

if __name__ == '__main__':
    main()
