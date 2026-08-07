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
PACKAGE_VERSION="$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)"
CURRENT_BRANCH="$(git branch --show-current)"

if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${RED}releases must be created from main (currently: ${CURRENT_BRANCH:-detached HEAD}).${NC}"
    exit 1
fi

if [ "$PACKAGE_VERSION" != "$VERSION" ]; then
    echo -e "${RED}Cargo.toml version is ${PACKAGE_VERSION:-missing}, not ${VERSION}.${NC}"
    echo "Update Cargo.toml and Cargo.lock before releasing."
    exit 1
fi

echo -e "${CYAN}==> starting release pipeline for haj ${TAG}...${NC}"

echo -e "\n${CYAN}==> [1/5] committing and tagging release...${NC}"
git add .

if git diff --cached --quiet; then
    echo "no uncommitted release changes; tagging the current main commit."
else
    git commit -m "release: ${TAG}"
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
    echo -e "${RED}tag ${TAG} already exists. Choose a new version or delete the local tag intentionally.${NC}"
    exit 1
fi

git tag -a "${TAG}" -m "release ${TAG}"

echo -e "pushing commits and tags to origin..."
git push origin main
git push origin "${TAG}"

echo -e "\n${CYAN}==> [2/5] Publishing to crates.io...${NC}"
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

echo -e "\n${CYAN}==> [3/5] Creating source release bundle...${NC}"
./scripts/package-source-release.sh "$VERSION"

echo -e "\n${CYAN}==> [4/5] Creating GitHub release...${NC}"
if command -v gh >/dev/null 2>&1; then
    read -p "Publish GitHub release ${TAG} with the source bundle? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        RELEASE_ASSETS=("dist/haj-${VERSION}-source.tar.gz")
        if [ -f "completions.tar.gz" ]; then
            RELEASE_ASSETS+=("completions.tar.gz")
        fi
        gh release create "$TAG" "${RELEASE_ASSETS[@]}" --title "haj ${TAG}" --generate-notes
        echo -e "${GREEN}GitHub release created successfully!${NC}"
    else
        echo -e "${RED}skipped GitHub release.${NC}"
    fi
else
    echo -e "${RED}GitHub CLI (gh) is not installed; skipped GitHub release.${NC}"
fi

echo -e "\n${CYAN}==> [5/5] Updating AUR package...${NC}"
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
