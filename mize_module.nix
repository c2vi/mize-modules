{ fetchgit,
fetchFromGitHub
, ...
}: {

externals = [

  (fetchFromGitHub {
    owner = "c2vi";
    repo = "mme";
    rev = "a04f4df5ce8e3b1a8d024320176fab84a4f4e8ed";
    hash = "sha256-vtcbIPypT4pRu6EwgKxWX/KjoAAYQHivQnZQRwY947M=";
  })

  /*
  (fetchFromGitHub {
    owner = "c2vi";
    repo = "victorinix";
    rev = "690aa83bc3b37263b1aeab61b3bd8f84960e0b1e";
    hash = "sha256-Xj/aESN3yeMMVfmL05HXt7gMLv+W4yNB8RcNSk6FweQ=";
  })
  */

];

}
