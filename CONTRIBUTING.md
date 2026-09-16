# Contributing to re_mp4
This guide is for anyone who wants to contribute to the `re_mp4` repository, for employees and outside contributors alike.

## What to contribute
* Report bugs and feature requests at <https://github.com/rerun-io/re_mp4/issues>.
* Look at our [`good first issue` tag](https://github.com/rerun-io/re_mp4/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22good%20first%20issue%22).

Note that maintainers do not have infinite time, and reviews take a lot of it.
When choosing what to work on, please ensure that it is either:

* A small change (+100-100 at most), or
* A larger change that has been discussed with one or more maintainers.

* Commenting on an existing issue,
* Creating a new issue, or
* Pinging one of the Rerun maintainers on our [Discord](https://discord.gg/PXtCgFBSmH)

> [!NOTE]
> PRs containing large undiscussed changes may be closed without comment.
> The same applies to issues and PRs opened by bot accounts (e.g. OpenClaw) or that are clearly agent-generated without human review — see [Agents](#agents).

### PR draft
It can be useful to open a PR in _draft_ mode first. On reason is to get CI to run on it.

Another reason is to ask for early feedback on e.g. the user interaction or the overall design of the PR.
This can be a great way to discuss architectural ideas before doing the full work of implementing it.
If you want such early feedback, ask for it explicitly (e.g. ping someone relevant).

Do not un-draft until you have read all your code and _you_ think it is ready to merge.

An un-drafted PR means "ready for review".

### PR description
- Make sure the PR description is _inviting_ - not too long, not too short
- Write it yourself
- Describe _why_ you made this change (and link to any relevant issue/PR)
- If it makes sense, include an image or a video
- Describe what you want reviewed, e.g.
  - The UX — does this feature feel nice to use?
  - The architecture / design — explain the proposed design in the PR description, and keep the PR in draft mode
  - The code
- Express your own confidence in your work
  - Is this a simple fix for something you understand well, or maybe something well outside your domain that an agent wrote for you?

### Agents
Coding agents are powerful tools, but like any tool should be used wisely.

If you use an agent to prototype some feature, then the PR should be in draft mode, and you should ask for feedback on the _effect_ of the PR, rather than its contents.

If you use an agent to implement a solution, then you should be able to understand that solution.
Asking the agent to walk you through the code can help, but doesn't replace reading it yourself.
LLMs make it easy to produce code quickly, while understanding takes longer.
Please disclose the level of confidence that you have in your solution.
