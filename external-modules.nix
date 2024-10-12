{ fetchGit,
fetchFromGithub
, ...
}: [

  (fetchFromGithub {
    owner = "c2vi";
    repo = "mme";
    ref = "master";
    hash = "";
  })

  (fetchFromGithub {
    owner = "c2vi";
    repo = "vic";
    ref = "master";
    hash = "";
  })

]
