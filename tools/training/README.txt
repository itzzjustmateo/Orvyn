ORVYN 1 - EXPERIMENTAL TRAINING PACKAGE

Historical status: a CPU trial completed on October 30, 2026. The candidate was
not suitable for activation. It ran 24 LoRA steps with four CPU threads.
Training and before/after evaluation took 188 seconds after model loading;
environment preparation and loading took additional time. The active app model
was not replaced.

Candidate: models/training/orvyn1-candidate-20261030-190726
Results: evaluation.json, answers.json, and review.json in that folder.

Held-out loss changed from 1.530 to 1.541 for Dutch, 3.499 to 2.820 for code,
2.179 to 1.907 for web, and 3.313 to 2.033 for Opex. Lower loss is better, but
the four generated answers did not show reliable improvement: code answered a
different question, citations were missing, and the Opex read action was invalid.
Do not merge or activate this candidate.

The original seed set contained 48 training and 16 evaluation examples. The v2
dataset at data/training/orvyn-1/v2 contains 112 training and 48 evaluation
examples. Original JSONL files are retained. The additions target instruction
following, citations, and Opex action formatting. Twenty-four Python reference
functions were checked against 36 input/output cases. No new model has yet been
trained on v2. These are synthetic examples, not private conversations. Dataset
validation does not prove model improvement. Extend the set with reviewed,
varied examples and independent practical evaluation.

The dataset intentionally contains Dutch-language training and evaluation
examples. Those examples retain their original language so their evaluation
purpose remains meaningful. Application controls and documentation use English.

Historical hardware: 32 GB RAM, Intel UHD Graphics 630, no CUDA GPU. The CPU trial
used about 5.2 GB RAM. The existing system had PyTorch 2.14 CPU and Transformers
5.17. New environments and caches remain on the USB; system software is not
modified.

Settings > Training shows candidates, examples, and before/after answers.
Settings > Startup shows timings for the current app launch. New trial progress
appears in logs/training/progress.json. Old progress does not prove a process is
still running.

Validate without training (--dataset seed selects the original set):
  python tools/training/train_orvyn.py

Validate data and reference functions:
  python tools/training/check_data.py --dataset v2
  python tests/test_training.py

Prepare the environment (missing packages are installed only under tools):
  powershell -File scripts/setup-training.ps1

Run a new isolated CPU trial:
  tools/training-venv/Scripts/python.exe tools/training/train_orvyn.py --train --device cpu --steps 24 --threads 4

The script reads the existing GGUF and dequantizes it to float32, requiring more
RAM than normal chat. Only LoRA adapters are trained; original weights remain
intact. Held-out loss is measured per domain before and after training. Review
generated answers and tool proposals as well as loss. PC training grants no
additional permissions, mouse control, or new tools.

Output is stored under models/training/orvyn1-candidate-*. Adapters are not
registered in the app automatically. A successful candidate still requires
content review, merging/conversion to a compatible GGUF, and comparison with
the current model. Nothing is sent to a cloud training service and the script
does not incur cloud training charges.

Design references:
https://huggingface.co/docs/transformers/gguf
https://huggingface.co/docs/trl/peft_integration
