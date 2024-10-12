{ fetchgit,
fetchFromGitHub
, ...
}: [

  (fetchFromGitHub {
    owner = "c2vi";
    repo = "mme";
    rev = "bf807dab7ca3374e0edc79dd4895895aa0b465b8";
    hash = "sha256-0N3Crgy3aQVKB3/uQmHlN63kP3imwPuHW2+G022oNfI=";
  })

  (fetchFromGitHub {
    owner = "c2vi";
    repo = "victorinix";
    rev = "690aa83bc3b37263b1aeab61b3bd8f84960e0b1e";
    hash = "sha256-Xj/aESN3yeMMVfmL05HXt7gMLv+W4yNB8RcNSk6FweQ=";
  })

]
