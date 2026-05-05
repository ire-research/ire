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

## Experiment dispatch logic
Run the following prompt:
```
Let's test the experiment workflow. Write a minimal python script to do the following: for each i in range(10), sleep 3 seconds and print (iteration nr. i), then finish.
```

## Fronted Chat Rendering

Run the following to generate as many tools, thinking and text as possible.
```
I am testing IRE's frontend rendering, which wraps your answers with a custom UI. To help, please respond by including some thinking, text and multiple tools usage. Maintain the reponse brief to avoid overspending token, but ensure variety in the content (i.e., use thinking, tools and plain text).
```