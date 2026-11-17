// The little of Linear's GraphQL API this needs. `fetch` is passed in so a test can answer for it.

export class Linear {
  constructor(apiKey, fetchImpl = globalThis.fetch, url = 'https://api.linear.app/graphql') {
    this.apiKey = apiKey;
    this.fetch = fetchImpl;
    this.url = url;
  }

  async call(query, variables = {}) {
    // A personal API key is sent as it is; an OAuth token as a bearer.
    const authorization = this.apiKey.startsWith('lin_oauth_') ? `Bearer ${this.apiKey}` : this.apiKey;
    const res = await this.fetch(this.url, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization },
      body: JSON.stringify({ query, variables }),
    });
    if (!res.ok) throw new Error(`Linear answered ${res.status}: ${(await res.text()).slice(0, 200)}`);
    const body = await res.json();
    if (body.errors?.length) throw new Error(`Linear: ${body.errors.map((e) => e.message).join('; ')}`);
    return body.data;
  }

  async team(key) {
    const d = await this.call(
      `query($key: String!) { teams(filter: { key: { eq: $key } }) { nodes { id key states { nodes { id name } } } } }`,
      { key },
    );
    const team = d.teams.nodes[0];
    if (!team) throw new Error(`no Linear team with the key ${key}`);
    return { id: team.id, states: new Map(team.states.nodes.map((s) => [s.name, s.id])) };
  }

  /** The issues carrying `label` in the team, every page. */
  async issues(teamId, label) {
    const out = [];
    let after = null;
    do {
      const d = await this.call(
        `query($team: ID!, $label: String!, $after: String) {
           issues(first: 100, after: $after, filter: { team: { id: { eq: $team } }, labels: { name: { eq: $label } } }) {
             nodes { id identifier title description state { id name } labels { nodes { id name } } }
             pageInfo { hasNextPage endCursor }
           }
         }`,
        { team: teamId, label, after },
      );
      for (const n of d.issues.nodes) out.push({ ...n, labels: n.labels.nodes });
      after = d.issues.pageInfo.hasNextPage ? d.issues.pageInfo.endCursor : null;
    } while (after);
    return out;
  }

  /** Label ids by name, creating the ones that are missing. */
  async labels(teamId, names, create = true) {
    const d = await this.call(`query { issueLabels(first: 250) { nodes { id name team { id } } } }`);
    const have = new Map(
      d.issueLabels.nodes.filter((l) => !l.team || l.team.id === teamId).map((l) => [l.name, l.id]),
    );
    for (const name of names) {
      if (have.has(name) || !create) continue;
      const made = await this.call(
        `mutation($input: IssueLabelCreateInput!) { issueLabelCreate(input: $input) { success issueLabel { id name } } }`,
        { input: { name, teamId } },
      );
      have.set(name, made.issueLabelCreate.issueLabel.id);
    }
    return have;
  }

  async create(input) {
    const d = await this.call(
      `mutation($input: IssueCreateInput!) { issueCreate(input: $input) { success issue { id identifier url } } }`,
      { input },
    );
    return d.issueCreate.issue;
  }

  async update(id, input) {
    await this.call(
      `mutation($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success } }`,
      { id, input },
    );
  }

  async comment(issueId, body) {
    await this.call(
      `mutation($input: CommentCreateInput!) { commentCreate(input: $input) { success } }`,
      { input: { issueId, body } },
    );
  }
}
