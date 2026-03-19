#!/usr/bin/env bash
set -e

cleanup() {
  kill $CARGO_PID $VITE_PID 2>/dev/null
  wait $CARGO_PID $VITE_PID 2>/dev/null
}
trap cleanup EXIT

cd "$(dirname "$0")"

echo "Starting axum server on :8080..."
cargo run -p fml9000-web &
CARGO_PID=$!

# Wait for axum to be ready
for i in $(seq 1 30); do
  if curl -s http://localhost:8080/api/playback/state >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

echo "Starting vite dev server on :5173..."
cd frontend
pnpm dev &
VITE_PID=$!

echo ""
echo "  Open http://localhost:5173 for development (HMR + proxy to axum)"
echo "  Axum API running on http://localhost:8080"
echo ""

wait
