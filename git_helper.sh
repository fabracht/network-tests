#!/bin/bash

# Define function to list submodules
function list_submodules() {
  echo "List of submodules:"
  git submodule foreach --quiet 'echo $path'
  echo ""
}

# Define function to display help menu
function help_menu() {
  echo "Usage: ./git_helper.sh [options]"
  echo ""
  echo "Options:"
  echo "  -l, --list          List submodules"
  echo "  -s, --submodule     Commit and push changes for a specific submodule"
  echo "  -u, --update        Update submodules"
  echo "  -m, --message       Specify commit message (use with -s or -b)"
  echo "  -b, --bulk          Commit and push changes for a list of space-separated submodules (use with -m)"
  echo "  -p, --push-base     Commit and push changes for the base project (main repository)"
  echo "  -h, --help          Display this help menu"
  echo ""
  echo "Examples:"
  echo "  ./git_helper.sh -l               # List submodules"
  echo "  ./git_helper.sh -s <submodule-name> -m \"Your commit message\"  # Commit and push changes for a specific submodule with a custom commit message"
  echo "  ./git_helper.sh -u               # Update submodules"
  echo "  ./git_helper.sh -b \"submodule1 submodule2\" -m \"Your commit message\"  # Commit and push changes for a list of space-separated submodules with a custom commit message"
  echo "  ./git_helper.sh -p -m \"Your commit message\"  # Commit and push changes for the base project (main repository) with a custom commit message"
  echo ""
}

# Initialize variables
commit_message=""
bulk_submodules=""
push_base=false

# Check if help option was passed or no arguments provided
if [[ $1 == "-h" || $1 == "--help" || $# -eq 0 ]]; then
  help_menu
  exit 0
fi

# Check for flags
while [[ "$#" -gt 0 ]]; do
  case $1 in
    -l|--list) list_submodules; exit 0 ;;
    -m|--message) commit_message="$2"; shift ;;
    -b|--bulk) bulk_submodules="$2"; shift ;;
    -p|--push-base) push_base=true ;;
    -s|--submodule)
      # Check if submodule name is provided
      if [[ -z $2 ]]; then
        echo "Error: Submodule name not provided. Usage: ./git_helper.sh -s <submodule-name>"
        exit 1
      fi
      
      # Check if commit message is provided
      if [[ -z $commit_message ]]; then
        commit_message="Commit message in $(basename "$(pwd)") submodule"
      fi

      # Commit changes for the specified submodule and push to remote
      cd "$2"
      echo "Committing changes in $(basename "$(pwd)") submodule"
      git add .
      git commit -m "$commit_message"
      git push
      echo "Changes in $(basename "$(pwd)") submodule committed and pushed to remote"
      cd ..
      exit 0 ;;
    -u|--update)
      echo "Updating submodules"
            git submodule update --recursive --remote
      echo "Submodules updated"
      exit 0 ;;
    *) echo "Unknown option: $1"; help_menu; exit 1 ;;
  esac
  shift
done

# Process bulk submodules if specified
if [[ -n $bulk_submodules ]]; then
  # Check if commit message is provided
  if [[ -z $commit_message ]]; then
    commit_message="Commit message in bulk submodules"
  fi

  # Commit changes for the specified submodules and push to remote
  for submodule in $bulk_submodules; do
    cd "$submodule"
    echo "Committing changes in $(basename "$(pwd)") submodule"
    git add .
    git commit -m "$commit_message"
    git push
    echo "Changes in $(basename "$(pwd)") submodule committed and pushed to remote"
    cd ..
  done
fi

# Commit and push changes for the base project if requested
if [[ $push_base == true ]]; then
  # Check if commit message is provided
  if [[ -z $commit_message ]]; then
    commit_message="Commit message in base project"
  fi

  echo "Committing changes in the base project"
  git add .
  git commit -m "$commit_message"
  git push
  echo "Changes in the base project committed and pushed to remote"
fi
