#!/usr/bin/env bash
#
# Generate MBTiles for Bavaria using Planetiler
# Planetiler is a faster, simpler alternative to OpenMapTiles that works well on NixOS
#
# Usage:
#   ./generate-bavaria-planetiler.sh                    # Generate with default settings
#   MAX_ZOOM=16 ./generate-bavaria-planetiler.sh        # Higher detail
#

set -e

# Configuration
REGION="bayern"
MIN_ZOOM="${MIN_ZOOM:-0}"
MAX_ZOOM="${MAX_ZOOM:-15}"
OSM_URL="https://download.geofabrik.de/europe/germany/bayern-latest.osm.pbf"
OUTPUT_FILE="bavaria.mbtiles"
BACKUP_FILE="bavaria-$(date +%Y%m%d).mbtiles.backup"
PLANETILER_VERSION="${PLANETILER_VERSION:-latest}"
BBOX="8.9771,47.2701,13.8339,50.5647"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

# Check Docker
if ! command -v docker &> /dev/null; then
    log_error "Docker is required but not installed!"
    exit 1
fi

log_info "=== Bavaria MBTiles Generator using Planetiler ==="
log_info "Region: Bayern (Bavaria), Germany"
log_info "Zoom levels: $MIN_ZOOM to $MAX_ZOOM"
log_info "Output: $OUTPUT_FILE"
log_info ""
log_info "Planetiler is faster and simpler than OpenMapTiles!"
log_info "Expected time: 10-30 minutes (depending on zoom level)"
log_info ""

# Ask for confirmation (skip if SKIP_CONFIRM is set)
if [ -z "$SKIP_CONFIRM" ]; then
    read -p "Continue? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Cancelled."
        exit 0
    fi
fi

# Create data directory
mkdir -p data
cd data

# Download OSM data if not exists
OSM_FILE="bayern-latest.osm.pbf"
if [ ! -f "$OSM_FILE" ]; then
    log_info "Downloading Bavaria OSM data from Geofabrik (~400MB)..."
    curl -L -o "$OSM_FILE" "$OSM_URL"
    log_success "Download complete!"
else
    log_info "Using existing OSM data: $OSM_FILE"
    log_info "Delete this file to download fresh data"
fi

cd ..

# Run Planetiler in Docker (rootless - as current user)
log_info "Running Planetiler to generate tiles..."
log_info "This will take 10-30 minutes depending on zoom level and system..."

docker run --user $(id -u):$(id -g) -v "$(pwd)/data:/data" ghcr.io/onthegomap/planetiler:$PLANETILER_VERSION \
    --download \
    --area=bayern \
    --osm-path=/data/$OSM_FILE \
    --output=/data/output.mbtiles \
    --bounds=$BBOX \
    --minzoom=$MIN_ZOOM \
    --maxzoom=$MAX_ZOOM

# Check if output exists
if [ ! -f "data/output.mbtiles" ]; then
    log_error "Generation failed - output file not found!"
    exit 1
fi

# Backup existing file if it exists
if [ -f "$OUTPUT_FILE" ]; then
    log_info "Backing up existing $OUTPUT_FILE to $BACKUP_FILE"
    mv "$OUTPUT_FILE" "$BACKUP_FILE"
fi

# Move to final location
mv data/output.mbtiles "$OUTPUT_FILE"

# Get file info
FILE_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)

log_success "=== Generation Complete! ==="
log_success "Output: $OUTPUT_FILE"
log_success "Size: $FILE_SIZE"

# Fix ownership if files were created as root
if [ ! -w "$OUTPUT_FILE" ]; then
    log_warning "Output file owned by root, attempting to fix permissions..."
    docker run --rm -v "$(pwd):/work" alpine chown -R $(id -u):$(id -g) /work/$OUTPUT_FILE /work/data 2>/dev/null || true
fi

# Cleanup data
log_info "Cleaning up temporary data..."
rm -rf data/sources data/tmp 2>/dev/null || log_warning "Some files require manual cleanup (Docker root owned)"
log_info "Cleaning up downloaded OSM data..."
rm -f "data/$OSM_FILE" 2>/dev/null || true
log_success "Cleanup complete!"

log_info ""
log_info "To use this file with TileServer GL:"
log_info "  Restart the tile server:"
log_info "    docker compose down && docker compose up -d"
log_info ""

if [ -f "$BACKUP_FILE" ]; then
    log_info "Previous version backed up as: $BACKUP_FILE"
    log_info "Delete it to free up space if the new tiles work correctly."
fi

log_success "Done! Ready to serve immediately."
