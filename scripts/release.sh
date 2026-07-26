#!/usr/bin/env bash
set -e

CYAN='\033[36m'
GREEN='\033[32m'
RED='\033[31m'
NC='\033[0m' 

if [ -z "$1" ]; then
    echo -e "${RED}error: version number not provided.${NC}"
    echo "Usage: ./release.sh 0.2.3"
    exit 1
fi

VERSION="$1"
TAG="v$VERSION"
DEFAULT_AUR_PATH="/home/asitos/Projects/haj-aur/haj"

echo -e "${CYAN}==> starting release pipeline for haj ${TAG}...${NC}"

echo -e "\n${CYAN}==> [1/4] committing and tagging release...${NC}"
git add .
git commit -m "release: ${TAG}"
git tag -a "${TAG}" -m "release ${TAG}"

echo -e "pushing commits and tags to origin..."
git push origin main
git push origin "${TAG}"

echo -e "\n${CYAN}==> [2/4] Publishing to crates.io...${NC}"
echo "Running dry-run..."
cargo publish --dry-run

read -p "dry-run successful. proceed with live crates.io publish? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cargo publish
    echo -e "${GREEN}successfully published to crates.io. Waiting 30s for indexing...${NC}"
    sleep 30
else
    echo -e "${RED}skipped crates.io publish.${NC}"
fi

echo -e "\n${CYAN}==> [3/4] Updating AUR package...${NC}"
read -p "Enter path to local AUR clone [default: $DEFAULT_AUR_PATH] (or type 'skip'): " AUR_PATH
AUR_PATH="${AUR_PATH:-$DEFAULT_AUR_PATH}"

if [ "$AUR_PATH" != "skip" ] && [ -d "$AUR_PATH" ]; then
    cd "$AUR_PATH"
    
    sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD
    sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD
    
    echo "updating checksums..."
    updpkgsums
    
    echo "generating .SRCINFO..."
    makepkg --printsrcinfo > .SRCINFO
    
    echo "verifying package build..."
    makepkg
    
    echo "committing AUR changes..."
    git add PKGBUILD .SRCINFO
    git commit -m "upgpkg: haj ${VERSION}-1"
    git push
    
    echo -e "${GREEN}AUR package updated successfully!${NC}"
else
    echo -e "${RED}AUR path skipped or invalid.${NC}"
fi

echo -e "\n${GREEN}==> release ${TAG} pipeline completed successfully!${NC}"
