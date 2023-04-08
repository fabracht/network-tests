#!/bin/bash

# Check if jq is installed, and if not, prompt the user to install it
if ! command -v jq &> /dev/null
then
    echo "jq is not installed."
    read -p "Do you want to install jq? (y/n) " choice
    case "$choice" in
        y|Y ) sudo apt-get install jq;;
        n|N ) echo "Exiting without installing jq."; exit;;
        * ) echo "Invalid choice. Exiting without installing jq."; exit;;
    esac
fi

# Set your API key as an environment variable
export OPENAI_API_KEY=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

# Set the parameters for the API request
MODEL="text-davinci-002"
MAX_TOKENS=30

# Generate the git diff for uncommitted changes with added/modified/removed lines and relative paths
changes=$(git diff --no-commit-id --diff-filter=AMR --submodule=diff --unified=0 --relative )

# Escape double quotes and newlines in the changes variable
changes_escaped=$(echo "$changes" | sed -e 's/"/\\"/g' -e ':a;N;$!ba;s/\n/\\n/g')
# echo "$changes_escaped"
# Send the API request with the changes as input
response=$(curl -s -X POST https://api.openai.com/v1/chat/completions \
-H "Content-Type: application/json" \
-H "Authorization: Bearer $OPENAI_API_KEY" \
-d '{
  "model": "gpt-3.5-turbo",
  "messages": [{"role": "user", "content": "What are the latest changes in the repository? Provide the answer as a DESCRIPTION list for a commit message\n\n'"$changes_escaped"'"}],
  "temperature": 0.7
}')

echo "$response"
# Extract the assistant's message content from the response
summary=$(echo "$response" | jq -r '.choices[0].message.content')

# Print the summary of changes
echo "DESCRIPTION:"
echo "$summary"