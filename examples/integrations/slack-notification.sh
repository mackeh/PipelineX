#!/bin/bash
# Slack notification script for PipelineX analysis
# Usage: ./slack-notification.sh <workflow-file> <webhook-url>

set -e

WORKFLOW_FILE="${1:-.github/workflows/ci.yml}"
SLACK_WEBHOOK="${2:-$SLACK_WEBHOOK_URL}"

if [ -z "$SLACK_WEBHOOK" ]; then
    echo "Error: SLACK_WEBHOOK_URL not set"
    echo "Usage: $0 <workflow-file> <webhook-url>"
    exit 1
fi

# Run PipelineX analysis
echo "Analyzing $WORKFLOW_FILE..."
ANALYSIS_JSON=$(pipelinex analyze "$WORKFLOW_FILE" --format json)

# Extract key metrics
CRITICAL_ISSUES=$(echo "$ANALYSIS_JSON" | jq '[.findings[] | select(.severity == "Critical")] | length')
HIGH_ISSUES=$(echo "$ANALYSIS_JSON" | jq '[.findings[] | select(.severity == "High")] | length')
MEDIUM_ISSUES=$(echo "$ANALYSIS_JSON" | jq '[.findings[] | select(.severity == "Medium")] | length')
ESTIMATED_DURATION=$(echo "$ANALYSIS_JSON" | jq -r '.total_estimated_duration_secs // 0')
OPTIMIZED_DURATION=$(echo "$ANALYSIS_JSON" | jq -r '.optimized_duration_secs // 0')

# Calculate improvement
if [ "$OPTIMIZED_DURATION" != "0" ] && [ "$OPTIMIZED_DURATION" != "null" ]; then
    IMPROVEMENT=$(echo "scale=1; (1 - $OPTIMIZED_DURATION / $ESTIMATED_DURATION) * 100" | bc)
else
    IMPROVEMENT="N/A"
fi

# Determine color based on critical issues
if [ "$CRITICAL_ISSUES" -gt 0 ]; then
    COLOR="#FF0000"  # Red
    EMOJI="🚨"
elif [ "$HIGH_ISSUES" -gt 0 ]; then
    COLOR="#FFA500"  # Orange
    EMOJI="⚠️"
else
    COLOR="#00FF00"  # Green
    EMOJI="✅"
fi

# Create Slack message
cat > /tmp/slack-message.json <<EOF
{
  "text": "$EMOJI Pipeline Analysis Report",
  "blocks": [
    {
      "type": "header",
      "text": {
        "type": "plain_text",
        "text": "$EMOJI PipelineX Analysis Report"
      }
    },
    {
      "type": "section",
      "fields": [
        {
          "type": "mrkdwn",
          "text": "*Pipeline:*\n\`$WORKFLOW_FILE\`"
        },
        {
          "type": "mrkdwn",
          "text": "*Issues:*\n🔴 $CRITICAL_ISSUES Critical\n🟠 $HIGH_ISSUES High\n🟡 $MEDIUM_ISSUES Medium"
        }
      ]
    },
    {
      "type": "section",
      "fields": [
        {
          "type": "mrkdwn",
          "text": "*Current Duration:*\n$(echo "scale=1; $ESTIMATED_DURATION / 60" | bc) minutes"
        },
        {
          "type": "mrkdwn",
          "text": "*Potential Improvement:*\n$IMPROVEMENT%"
        }
      ]
    },
    {
      "type": "actions",
      "elements": [
        {
          "type": "button",
          "text": {
            "type": "plain_text",
            "text": "View Full Report"
          },
          "url": "https://github.com/$GITHUB_REPOSITORY/actions"
        }
      ]
    }
  ]
}
EOF

# Send to Slack
curl -X POST -H 'Content-type: application/json' \
    --data @/tmp/slack-message.json \
    "$SLACK_WEBHOOK"

echo "✓ Notification sent to Slack"
rm /tmp/slack-message.json
