# Test Prompts

## Workspace Init

Run the following to see CC populate the workspace according to your research needs:
```
Hello, let's get everything set up in the IRE workspace. I am working on diffusion language models, specifically I want to understand if they natively encode output length information in their latent space without explicit training. I am trying to set up LLADA 8B to run locally now.
```
Expected output: CC should populate NOTES.md, IDEAS.md, and PULSE.md with relevant material.

## Resource ingestion

Run the following to test resource ingestion. If the workspace is initialized as above, even better.
```
# paste inside the ingestion field and click submit
https://arxiv.org/pdf/2603.06123
```
CC should read the PDF and provide a summary with relevant information in a new chat tab. You can say confirm to index it.
