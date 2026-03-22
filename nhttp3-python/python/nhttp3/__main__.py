"""
nhttp3 CLI — serve ASGI apps over HTTP/3.

Usage:
    python -m nhttp3 run myapp:app --port 4433 --certfile cert.pem --keyfile key.pem

    # Or equivalently:
    nhttp3 run myapp:app --port 4433

This is the HTTP/3 equivalent of:
    uvicorn myapp:app --port 8000
"""

import argparse
import importlib
import sys


def main():
    parser = argparse.ArgumentParser(
        prog="nhttp3",
        description="HTTP/3 ASGI server powered by Rust",
    )
    subparsers = parser.add_subparsers(dest="command")

    # nhttp3 run <app> [options]
    run_parser = subparsers.add_parser("run", help="Serve an ASGI application")
    run_parser.add_argument("app", help="ASGI app path (e.g., myapp:app)")
    run_parser.add_argument("--host", default="0.0.0.0")
    run_parser.add_argument("--port", type=int, default=4433)
    run_parser.add_argument("--certfile", default=None)
    run_parser.add_argument("--keyfile", default=None)
    run_parser.add_argument("--workers", type=int, default=1)

    # nhttp3 version
    subparsers.add_parser("version", help="Show version")

    args = parser.parse_args()

    if args.command == "version":
        from nhttp3 import __version__

        print(f"nhttp3 {__version__}")
        return

    if args.command == "run":
        # Load the ASGI app
        module_path, _, attr = args.app.partition(":")
        if not attr:
            attr = "app"

        try:
            module = importlib.import_module(module_path)
            app = getattr(module, attr)
        except (ImportError, AttributeError) as e:
            print(f"Error loading {args.app}: {e}", file=sys.stderr)
            sys.exit(1)

        from nhttp3 import serve

        print(f"nhttp3 serving {args.app} on {args.host}:{args.port}")
        if args.certfile:
            print(f"  TLS: {args.certfile} / {args.keyfile}")
        print(f"  Protocol: HTTP/3 (QUIC)")
        print(f"  Press Ctrl+C to stop")
        print()

        serve(
            app,
            host=args.host,
            port=args.port,
            certfile=args.certfile,
            keyfile=args.keyfile,
        )
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
