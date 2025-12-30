#!/bin/bash
set -e

echo "Running database migrations..."
./diesel migration run

echo "Starting backend server..."
exec ./backend
