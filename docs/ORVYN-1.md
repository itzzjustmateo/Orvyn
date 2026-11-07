# Orvyn 1

Orvyn 1 is a local chat profile using the existing Llama 3.2 1B Instruct Q4_K_M weights (807,694,464 bytes). It is not a newly trained foundation model. The profile shares the existing GGUF file rather than downloading another copy.

The current profile requests concise, direct answers in English by default. Generation uses up to 512 response tokens, temperature 0.3, and top_p 0.9. A lower requested token limit is preserved. Larger models are better suited to long code files and complex reasoning. Actual speed depends on the PC and USB loading speed.

Select Orvyn 1 from the model selector or AI library after restarting. Its configuration is in `config/custom-models.json`, including the base model, source, and verification hash.

## Websites and performance

Include a public HTTPS link in a question, such as “Summarize https://example.com”. The app reads up to two links, supplies text excerpts to the model, and displays sources. This is not an interactive browser: sign-in, JavaScript applications, PDFs, and IPv6-only sites are unsupported.

The website receives the URL request, not the full chat. Pages are cached in memory for five minutes. Private and local IP addresses are blocked.

Orvyn 1 uses one server conversation slot rather than four, clear base instructions, and a maximum of 512 response tokens. A loaded, reachable model skips repeated startup and curl health checks. Initial startup still depends on USB and Windows file caches.

A historical live test on October 30, 2026 measured 45.49 seconds for the first response including loading, and 0.195 seconds for a short follow-up. These are measurements from one PC, not guaranteed timings. Website reading, caching, and local-address blocking were also tested.
